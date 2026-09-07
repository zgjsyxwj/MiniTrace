//! 采集工作区磁盘水位探测。
//!
//! 磁盘水位是风险提示，不是停止条件。探测模块只读取文件系统容量，调用方
//! 负责把警告追加到当前会话并持续展示；这里绝不删除历史会话或终止 worker。

use crate::model::DiskSpaceStatus;
use std::path::Path;
use std::process::Command;

/// 低于该比例或该绝对剩余空间时显示磁盘水位警告。
pub const WARNING_RATIO_PERCENT: u64 = 10;
pub const WARNING_FREE_BYTES: u64 = 512 * 1024 * 1024;

/// 从容量数值构造稳定、可测试的磁盘状态。
pub fn status_from_capacity(
    path: &Path,
    free_bytes: u64,
    total_bytes: u64,
    checked_at_ms: u64,
) -> DiskSpaceStatus {
    let ratio_warning = total_bytes > 0
        && (free_bytes as u128) * 100 <= (total_bytes as u128) * WARNING_RATIO_PERCENT as u128;
    let warning = ratio_warning || free_bytes <= WARNING_FREE_BYTES;
    DiskSpaceStatus {
        path: path.to_string_lossy().into_owned(),
        free_bytes,
        total_bytes,
        warning,
        warning_ratio_percent: WARNING_RATIO_PERCENT,
        warning_free_bytes: WARNING_FREE_BYTES,
        checked_at_ms,
    }
}

/// 读取工作区所在文件系统的可用空间。
///
/// Unix 使用不带 shell 的 `df -Pk`，参数直接传给进程，因此工作区路径中的
/// 空格或 shell 字符不会被解释。Windows 使用系统 API；其他平台尝试同样的
/// `df` 只读命令，失败时返回明确错误。
pub fn inspect(path: &Path, checked_at_ms: u64) -> Result<DiskSpaceStatus, String> {
    if !path.is_absolute() {
        return Err("磁盘水位探测需要绝对路径".to_string());
    }

    #[cfg(windows)]
    {
        return inspect_windows(path, checked_at_ms);
    }

    #[cfg(not(windows))]
    inspect_df(path, checked_at_ms)
}

#[cfg(not(windows))]
fn inspect_df(path: &Path, checked_at_ms: u64) -> Result<DiskSpaceStatus, String> {
    let output = Command::new("df")
        .args(["-Pk"])
        .arg(path)
        .output()
        .map_err(|error| format!("读取工作区磁盘水位失败：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "读取工作区磁盘水位失败，df 退出码 {:?}",
            output.status.code()
        ));
    }

    // POSIX `df -P` 的最后一行至少包含 filesystem、1024-blocks、used、
    // available、capacity、mounted-on。挂载点可能包含空格，因此从右侧
    // 读取固定的容量列，避免依赖挂载点的空格分割。
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rfind(|line| !line.trim().is_empty())
        .ok_or_else(|| "df 没有返回工作区文件系统信息".to_string())?;
    let (total_blocks, free_blocks) = parse_df_capacity_line(line)?;
    Ok(status_from_capacity(
        path,
        free_blocks.saturating_mul(1024),
        total_blocks.saturating_mul(1024),
        checked_at_ms,
    ))
}

#[cfg(not(windows))]
fn parse_df_capacity_line(line: &str) -> Result<(u64, u64), String> {
    let fields = line.split_whitespace().collect::<Vec<_>>();
    let capacity_index = fields
        .iter()
        .rposition(|field| field.ends_with('%'))
        .ok_or_else(|| format!("无法解析 df 容量百分比：{line}"))?;
    if capacity_index < 4 {
        return Err(format!("无法解析 df 输出：{line}"));
    }
    let total_index = capacity_index - 3;
    let free_index = capacity_index - 1;
    let total_blocks = fields
        .get(total_index)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| format!("无法解析 df 总容量：{line}"))?;
    let free_blocks = fields
        .get(free_index)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| format!("无法解析 df 可用容量：{line}"))?;
    Ok((total_blocks, free_blocks))
}

#[cfg(windows)]
fn inspect_windows(path: &Path, checked_at_ms: u64) -> Result<DiskSpaceStatus, String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            directory_name: *const u16,
            free_bytes_available: *mut u64,
            total_number_of_bytes: *mut u64,
            total_number_of_free_bytes: *mut u64,
        ) -> i32;
    }

    let mut directory = path.as_os_str().encode_wide().collect::<Vec<_>>();
    directory.push(0);
    let mut free = 0_u64;
    let mut total = 0_u64;
    let mut total_free = 0_u64;
    // SAFETY: all pointers reference writable local integers and a NUL-terminated
    // UTF-16 path owned for the duration of the call.
    let ok =
        unsafe { GetDiskFreeSpaceExW(directory.as_ptr(), &mut free, &mut total, &mut total_free) };
    if ok == 0 {
        return Err(format!(
            "读取工作区磁盘水位失败：{}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(status_from_capacity(path, free, total, checked_at_ms))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[cfg(not(windows))]
    #[test]
    fn parses_df_available_blocks_before_capacity_even_when_mount_has_spaces() {
        let (total, free) =
            parse_df_capacity_line("/dev/disk1 100000 25000 75000 25% /Volumes/Capture Workspace")
                .expect("解析 df 行");
        assert_eq!(total, 100000);
        assert_eq!(free, 75000);
    }

    #[test]
    fn warns_when_absolute_free_space_is_below_safe_waterline() {
        let status = status_from_capacity(
            &PathBuf::from("/tmp/minitrace"),
            WARNING_FREE_BYTES,
            10 * 1024 * 1024 * 1024,
            7,
        );
        assert!(status.warning);
        assert_eq!(status.free_bytes, WARNING_FREE_BYTES);
    }

    #[test]
    fn warns_when_ratio_is_low_even_with_more_than_absolute_threshold() {
        let status = status_from_capacity(
            &PathBuf::from("/tmp/minitrace"),
            900 * 1024 * 1024,
            10 * 1024 * 1024 * 1024,
            8,
        );
        assert!(status.warning);
    }

    #[test]
    fn healthy_capacity_does_not_warn() {
        let status = status_from_capacity(
            &PathBuf::from("/tmp/minitrace"),
            5 * 1024 * 1024 * 1024,
            10 * 1024 * 1024 * 1024,
            9,
        );
        assert!(!status.warning);
    }
}
