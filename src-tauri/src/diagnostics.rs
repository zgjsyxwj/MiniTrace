//! macOS 环境诊断和候选进程选择。
//!
//! 诊断层只读取主机信息和进程信息，不附加进程、不创建会话，也不尝试
//! 使用未验证版本的旧采集配置。正式采集前必须再次通过这里的资格判断。

use crate::model::{
    CandidateProcess, DiagnosticIssue, DiagnosticIssueCode, DiagnosticMode, EnvironmentDiagnosis,
    SUPPORTED_MACOS_ARCHITECTURE, SUPPORTED_MACOS_WECHAT_VERSION,
};
use std::collections::HashSet;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub const REQUIRED_MODULES: &[&str] = &["WeChatAppEx Framework"];
pub const CAPTURE_BOUNDARY_MODULE_HINTS: &[&str] = &["flue", "xweb"];
pub const TARGET_PROCESS_ROLES: &[&str] = &["WeApp", "WeChatAppEx"];

/// `ps`/`vmmap` 等系统命令返回的只读进程快照。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedProcess {
    pub id: String,
    pub pid: u32,
    pub display_name: String,
    pub executable_path: String,
    pub process_role: String,
    pub game: String,
    pub wechat_version: String,
    pub architecture: String,
    pub modules: Vec<String>,
    pub backend: String,
}

/// 诊断输入和采集判定解耦，便于用固定假进程清单覆盖所有边界场景。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticInput {
    pub platform: String,
    pub os_version: String,
    pub architecture: String,
    pub wechat_version: String,
    pub processes: Vec<ObservedProcess>,
    pub inspection_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeDiagnosticScenario {
    Supported,
    UnknownVersion,
    MismatchedVersion,
    NoCandidate,
    MultipleCandidates,
}

impl FakeDiagnosticScenario {
    pub const ALL: [Self; 5] = [
        Self::Supported,
        Self::UnknownVersion,
        Self::MismatchedVersion,
        Self::NoCandidate,
        Self::MultipleCandidates,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::UnknownVersion => "unknown_version",
            Self::MismatchedVersion => "mismatched_version",
            Self::NoCandidate => "no_candidate",
            Self::MultipleCandidates => "multiple_candidates",
        }
    }
}

/// 在当前主机上执行只读 macOS 诊断。
pub fn diagnose_host() -> EnvironmentDiagnosis {
    #[cfg(target_os = "macos")]
    {
        let os_version = command_output("sw_vers", &["-productVersion"])
            .unwrap_or_else(|| "unknown".to_string());
        let architecture =
            command_output("uname", &["-m"]).unwrap_or_else(|| std::env::consts::ARCH.to_string());
        let wechat_version = detect_wechat_version().unwrap_or_else(|| "unknown".to_string());
        let (processes, inspection_error) = discover_processes(&wechat_version, &architecture);
        diagnose(DiagnosticInput {
            platform: "macOS".to_string(),
            os_version,
            architecture,
            wechat_version,
            processes,
            inspection_error,
        })
    }

    #[cfg(target_os = "windows")]
    {
        let (candidates, inspection_error) = match crate::windows_backend::list_candidates() {
            Ok(candidates) => (candidates, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let mut issues = vec![issue(
            DiagnosticIssueCode::PlatformIntegrationUnverified,
            "Windows 后端已接入，但当前微信版本尚未完成真实流量验收；仅提供兼容性诊断".to_string(),
            true,
        )];
        if let Some(error) = inspection_error {
            issues.push(issue(
                DiagnosticIssueCode::ProcessInspectionUnavailable,
                format!("读取 Windows 微信进程信息失败：{error}"),
                true,
            ));
        }
        if candidates.is_empty() {
            issues.push(issue(
                DiagnosticIssueCode::MissingCandidate,
                "没有发现带有 flue.dll 的 WeChatAppEx 或 WeApp 候选进程".to_string(),
                true,
            ));
        } else if candidates.len() > 1 {
            issues.push(issue(
                DiagnosticIssueCode::MultipleCandidates,
                format!(
                    "发现 {} 个 Windows 候选进程，请确认本次《世界 Online》运行链",
                    candidates.len()
                ),
                false,
            ));
        }
        let wechat_version = candidates
            .first()
            .map(|candidate| candidate.wechat_version.clone())
            .unwrap_or_else(|| "unknown".to_string());
        return EnvironmentDiagnosis {
            platform: "Windows".to_string(),
            os_version: "unknown".to_string(),
            architecture: candidates
                .first()
                .map(|candidate| candidate.architecture.clone())
                .unwrap_or_else(|| std::env::consts::ARCH.to_string()),
            wechat_version,
            mode: DiagnosticMode::CompatibilityDiagnostic,
            supported: false,
            capture_allowed: false,
            requires_candidate_confirmation: candidates.len() > 1,
            candidates,
            issues,
            required_modules: vec![crate::windows_backend::FLUE_MODULE_NAME.to_string()],
            checked_at_ms: now_ms(),
        };
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        diagnose(DiagnosticInput {
            platform: std::env::consts::OS.to_string(),
            os_version: "unknown".to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            wechat_version: "unknown".to_string(),
            processes: Vec::new(),
            inspection_error: Some("当前构建目标不是 macOS".to_string()),
        })
    }
}

/// 为真实 macOS runner 重新读取当前合格候选。
///
/// 这里只执行与启动前相同的只读诊断，不附加进程。恢复状态机仍会再次核对
/// 游戏、进程角色、微信版本、架构、后端和关键模块，避免把别的小游戏进程
/// 混入现有采集会话。
#[cfg(target_os = "macos")]
pub(crate) fn rediscover_macos_candidates() -> Vec<CandidateProcess> {
    diagnose_host().candidates
}

/// 对固定输入执行环境和候选资格判断。
pub fn diagnose(input: DiagnosticInput) -> EnvironmentDiagnosis {
    diagnose_at(input, now_ms())
}

/// 和 [`diagnose`] 相同，但允许测试固定诊断时间。
pub fn diagnose_at(input: DiagnosticInput, checked_at_ms: u64) -> EnvironmentDiagnosis {
    let platform_ok = is_macos(&input.platform);
    let architecture_ok = is_apple_silicon(&input.architecture);
    let (version_known, version_ok) = version_status(&input.wechat_version);
    let mut issues = Vec::new();

    if !platform_ok {
        issues.push(issue(
            DiagnosticIssueCode::UnsupportedPlatform,
            format!(
                "当前平台为 {}，正式 macOS 采集未启用",
                display_unknown(&input.platform)
            ),
            true,
        ));
    }
    if !architecture_ok {
        issues.push(issue(
            DiagnosticIssueCode::UnsupportedArchitecture,
            format!(
                "当前 CPU 架构为 {}，macOS 支持范围只包含 Apple Silicon",
                display_unknown(&input.architecture)
            ),
            true,
        ));
    }
    if !version_known {
        issues.push(issue(
            DiagnosticIssueCode::UnknownWechatVersion,
            "无法确认微信版本；未知版本只能进入兼容性诊断".to_string(),
            true,
        ));
    } else if !version_ok {
        issues.push(issue(
            DiagnosticIssueCode::MismatchedWechatVersion,
            format!(
                "微信版本 {} 未经过验证，当前仅支持 {}",
                display_unknown(&input.wechat_version),
                SUPPORTED_MACOS_WECHAT_VERSION
            ),
            true,
        ));
    }
    if let Some(error) = input.inspection_error.as_ref() {
        issues.push(issue(
            DiagnosticIssueCode::ProcessInspectionUnavailable,
            format!("读取微信进程信息失败：{error}"),
            true,
        ));
    }

    let environment_qualified = platform_ok && architecture_ok && version_ok;
    let mut candidates = Vec::new();
    let mut seen_ids = HashSet::new();
    for process in &input.processes {
        if !target_role(&process.process_role) {
            continue;
        }

        let process_architecture = if process.architecture.trim().is_empty() {
            input.architecture.as_str()
        } else {
            process.architecture.as_str()
        };
        if !is_apple_silicon(process_architecture) {
            continue;
        }

        let process_version = if process.wechat_version.trim().is_empty() {
            input.wechat_version.as_str()
        } else {
            process.wechat_version.as_str()
        };
        if version_status(process_version) != (true, true) {
            continue;
        }

        if !has_module(&process.modules, REQUIRED_MODULES[0]) {
            issues.push(issue(
                DiagnosticIssueCode::MissingRequiredModule,
                format!("候选 PID {} 未加载 {}", process.pid, REQUIRED_MODULES[0]),
                true,
            ));
            continue;
        }
        if !has_capture_boundary(&process.modules) {
            issues.push(issue(
                DiagnosticIssueCode::MissingCaptureBoundary,
                format!("候选 PID {} 未发现 Flue/XWeb 明文边界模块", process.pid),
                true,
            ));
            continue;
        }

        if !environment_qualified || !seen_ids.insert(process.id.clone()) {
            continue;
        }

        candidates.push(to_candidate(process, process_version, process_architecture));
    }
    candidates.sort_by(|left, right| left.pid.cmp(&right.pid).then(left.id.cmp(&right.id)));

    if candidates.is_empty() {
        issues.push(issue(
            DiagnosticIssueCode::MissingCandidate,
            "没有同时满足角色、微信版本和关键模块检查的候选进程".to_string(),
            true,
        ));
    } else if candidates.len() > 1 {
        issues.push(issue(
            DiagnosticIssueCode::MultipleCandidates,
            format!(
                "发现 {} 个合格候选进程，请确认本次《世界 Online》运行链",
                candidates.len()
            ),
            false,
        ));
    }

    let capture_allowed = environment_qualified && !candidates.is_empty();
    let mode = if capture_allowed {
        DiagnosticMode::Supported
    } else {
        DiagnosticMode::CompatibilityDiagnostic
    };

    EnvironmentDiagnosis {
        platform: input.platform,
        os_version: input.os_version,
        architecture: input.architecture,
        wechat_version: input.wechat_version,
        mode,
        supported: environment_qualified,
        capture_allowed,
        requires_candidate_confirmation: candidates.len() > 1,
        candidates,
        issues,
        required_modules: REQUIRED_MODULES
            .iter()
            .map(|module| (*module).to_string())
            .collect(),
        checked_at_ms,
    }
}

/// 返回用于自动化验收的固定假进程清单。
pub fn fake_input(scenario: FakeDiagnosticScenario) -> DiagnosticInput {
    let supported_process = || ObservedProcess {
        id: "fake-world-online-weapp".to_string(),
        pid: 42_013,
        display_name: "WeApp · 世界 Online（假后端）".to_string(),
        executable_path: "/Applications/WeChat.app/WeApp".to_string(),
        process_role: "WeApp".to_string(),
        game: "世界 Online".to_string(),
        wechat_version: SUPPORTED_MACOS_WECHAT_VERSION.to_string(),
        architecture: SUPPORTED_MACOS_ARCHITECTURE.to_string(),
        modules: vec![
            "WeChatAppEx Framework".to_string(),
            "flue.dylib".to_string(),
        ],
        backend: "fake".to_string(),
    };

    let mut processes = match scenario {
        FakeDiagnosticScenario::Supported => vec![supported_process()],
        FakeDiagnosticScenario::UnknownVersion => {
            let mut process = supported_process();
            process.wechat_version = "unknown".to_string();
            vec![process]
        }
        FakeDiagnosticScenario::MismatchedVersion => {
            let mut process = supported_process();
            process.wechat_version = "4.2.0".to_string();
            vec![process]
        }
        FakeDiagnosticScenario::NoCandidate => vec![ObservedProcess {
            id: "fake-wechat-helper".to_string(),
            pid: 42_012,
            display_name: "WeChat Helper（假清单）".to_string(),
            executable_path: "/Applications/WeChat.app/WeChat Helper".to_string(),
            process_role: "WeChatHelper".to_string(),
            game: String::new(),
            wechat_version: SUPPORTED_MACOS_WECHAT_VERSION.to_string(),
            architecture: SUPPORTED_MACOS_ARCHITECTURE.to_string(),
            modules: vec!["WeChatAppEx Framework".to_string()],
            backend: "fake".to_string(),
        }],
        FakeDiagnosticScenario::MultipleCandidates => {
            let first = supported_process();
            let mut second = supported_process();
            second.id = "fake-world-online-wechatappex".to_string();
            second.pid = 42_014;
            second.display_name = "WeChatAppEx · 世界 Online（假后端）".to_string();
            second.executable_path = "/Applications/WeChat.app/WeChatAppEx".to_string();
            second.process_role = "WeChatAppEx".to_string();
            vec![first, second]
        }
    };

    if matches!(scenario, FakeDiagnosticScenario::UnknownVersion) {
        // 环境版本未知时，进程本身也没有可信版本证据。
        processes[0].wechat_version = "unknown".to_string();
    }

    DiagnosticInput {
        platform: "macOS".to_string(),
        os_version: "14.6.1".to_string(),
        architecture: SUPPORTED_MACOS_ARCHITECTURE.to_string(),
        wechat_version: match scenario {
            FakeDiagnosticScenario::UnknownVersion => "unknown".to_string(),
            FakeDiagnosticScenario::MismatchedVersion => "4.2.0".to_string(),
            _ => SUPPORTED_MACOS_WECHAT_VERSION.to_string(),
        },
        processes,
        inspection_error: None,
    }
}

fn to_candidate(
    process: &ObservedProcess,
    process_version: &str,
    process_architecture: &str,
) -> CandidateProcess {
    CandidateProcess {
        id: process.id.clone(),
        pid: process.pid,
        display_name: process.display_name.clone(),
        process_role: process.process_role.clone(),
        game: if process.game.trim().is_empty() {
            "世界 Online".to_string()
        } else {
            process.game.clone()
        },
        wechat_version: process_version.trim().to_string(),
        architecture: canonical_architecture(process_architecture),
        modules: process.modules.clone(),
        backend: if process.backend.trim().is_empty() {
            "macos".to_string()
        } else {
            process.backend.clone()
        },
    }
}

fn issue(code: DiagnosticIssueCode, message: String, blocking: bool) -> DiagnosticIssue {
    DiagnosticIssue {
        code,
        message,
        blocking,
    }
}

fn target_role(role: &str) -> bool {
    TARGET_PROCESS_ROLES
        .iter()
        .any(|target| role.eq_ignore_ascii_case(target))
}

fn has_module(modules: &[String], expected: &str) -> bool {
    modules.iter().any(|module| {
        module
            .to_ascii_lowercase()
            .contains(&expected.to_ascii_lowercase())
    })
}

fn has_capture_boundary(modules: &[String]) -> bool {
    modules.iter().any(|module| {
        let lower = module.to_ascii_lowercase();
        CAPTURE_BOUNDARY_MODULE_HINTS
            .iter()
            .any(|hint| lower.contains(hint))
    })
}

fn is_macos(platform: &str) -> bool {
    matches!(
        platform.trim().to_ascii_lowercase().as_str(),
        "macos" | "darwin"
    )
}

fn is_apple_silicon(architecture: &str) -> bool {
    matches!(
        architecture.trim().to_ascii_lowercase().as_str(),
        "arm64" | "arm64e" | "aarch64"
    )
}

fn canonical_architecture(architecture: &str) -> String {
    if is_apple_silicon(architecture) {
        SUPPORTED_MACOS_ARCHITECTURE.to_string()
    } else if matches!(
        architecture.trim().to_ascii_lowercase().as_str(),
        "x86_64" | "amd64"
    ) {
        "x86_64".to_string()
    } else {
        architecture.trim().to_string()
    }
}

fn version_status(version: &str) -> (bool, bool) {
    let trimmed = version.trim();
    if trimmed.is_empty()
        || matches!(
            trimmed.to_ascii_lowercase().as_str(),
            "unknown" | "unknown version" | "n/a" | "na" | "未识别" | "未知"
        )
    {
        return (false, false);
    }
    (true, trimmed == SUPPORTED_MACOS_WECHAT_VERSION)
}

fn display_unknown(value: &str) -> &str {
    if value.trim().is_empty() {
        "未知"
    } else {
        value
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(target_os = "macos")]
fn discover_processes(
    wechat_version: &str,
    architecture: &str,
) -> (Vec<ObservedProcess>, Option<String>) {
    let output = match Command::new("ps")
        .args(["-axo", "pid=,ppid=,comm="])
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(output) => return (Vec::new(), Some(format!("ps 退出状态 {}", output.status))),
        Err(error) => return (Vec::new(), Some(error.to_string())),
    };

    let mut processes = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut fields = line.split_whitespace();
        let Some(pid) = fields.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(_parent_pid) = fields.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let executable_path = fields.collect::<Vec<_>>().join(" ");
        let Some(process_role) = process_role_from_path(&executable_path) else {
            continue;
        };
        let modules = inspect_modules(pid);
        let display_name = executable_path
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(&executable_path)
            .to_string();
        processes.push(ObservedProcess {
            id: format!("macos-{pid}"),
            pid,
            display_name,
            executable_path,
            process_role: process_role.to_string(),
            game: "世界 Online".to_string(),
            wechat_version: wechat_version.to_string(),
            architecture: architecture.to_string(),
            modules,
            backend: "macos".to_string(),
        });
    }
    (processes, None)
}

#[cfg(target_os = "macos")]
fn process_role_from_path(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    if lower.contains("weapp") {
        Some("WeApp")
    } else if lower.contains("wechatappex") {
        Some("WeChatAppEx")
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn inspect_modules(pid: u32) -> Vec<String> {
    let Ok(output) = Command::new("vmmap")
        .args(["-wide", &pid.to_string()])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let mut modules = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("wechatappex framework") {
            push_unique(&mut modules, "WeChatAppEx Framework".to_string());
        }
        if lower.contains("flue") {
            push_unique(&mut modules, "flue.dylib".to_string());
        }
        if lower.contains("xweb") {
            push_unique(&mut modules, "XWeb.framework".to_string());
        }
    }
    modules
}

#[cfg(target_os = "macos")]
fn detect_wechat_version() -> Option<String> {
    let paths = [
        "/Applications/WeChat.app/Contents/Info.plist",
        "/Applications/微信.app/Contents/Info.plist",
    ];
    for path in paths {
        if let Some(version) = command_output(
            "/usr/bin/plutil",
            &[
                "-extract",
                "CFBundleShortVersionString",
                "raw",
                "-o",
                "-",
                path,
            ],
        ) {
            return Some(version);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DiagnosticIssueCode, DiagnosticMode};

    #[test]
    fn supported_fixture_allows_one_candidate() {
        let report = diagnose_at(fake_input(FakeDiagnosticScenario::Supported), 1);
        assert_eq!(report.mode, DiagnosticMode::Supported);
        assert!(report.supported);
        assert!(report.capture_allowed);
        assert!(!report.requires_candidate_confirmation);
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].wechat_version, "4.1.13");
        assert_eq!(report.candidates[0].process_role, "WeApp");
        assert!(report.candidates[0]
            .modules
            .iter()
            .any(|module| module == "WeChatAppEx Framework"));
        let json = serde_json::to_value(&report).expect("序列化诊断结果");
        assert_eq!(json["osVersion"], "14.6.1");
        assert_eq!(json["wechatVersion"], "4.1.13");
        assert_eq!(json["captureAllowed"], true);
    }

    #[test]
    fn unknown_version_is_diagnostic_only_and_has_no_candidate() {
        let report = diagnose_at(fake_input(FakeDiagnosticScenario::UnknownVersion), 2);
        assert_eq!(report.mode, DiagnosticMode::CompatibilityDiagnostic);
        assert!(!report.capture_allowed);
        assert!(report.candidates.is_empty());
        assert!(report
            .issues
            .iter()
            .any(|item| item.code == DiagnosticIssueCode::UnknownWechatVersion));
    }

    #[test]
    fn mismatched_version_is_diagnostic_only_and_has_no_candidate() {
        let report = diagnose_at(fake_input(FakeDiagnosticScenario::MismatchedVersion), 7);
        assert_eq!(report.mode, DiagnosticMode::CompatibilityDiagnostic);
        assert!(!report.capture_allowed);
        assert!(report.candidates.is_empty());
        assert!(report
            .issues
            .iter()
            .any(|item| item.code == DiagnosticIssueCode::MismatchedWechatVersion));
    }

    #[test]
    fn no_candidate_is_diagnostic_only() {
        let report = diagnose_at(fake_input(FakeDiagnosticScenario::NoCandidate), 3);
        assert!(report.supported);
        assert_eq!(report.mode, DiagnosticMode::CompatibilityDiagnostic);
        assert!(!report.capture_allowed);
        assert!(report.candidates.is_empty());
        assert!(report
            .issues
            .iter()
            .any(|item| item.code == DiagnosticIssueCode::MissingCandidate));
    }

    #[test]
    fn multiple_candidates_require_explicit_selection() {
        let report = diagnose_at(fake_input(FakeDiagnosticScenario::MultipleCandidates), 4);
        assert_eq!(report.mode, DiagnosticMode::Supported);
        assert!(report.capture_allowed);
        assert!(report.requires_candidate_confirmation);
        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].pid, 42_013);
        assert_eq!(report.candidates[1].pid, 42_014);
        assert!(report
            .issues
            .iter()
            .any(|item| item.code == DiagnosticIssueCode::MultipleCandidates && !item.blocking));
    }

    #[test]
    fn x86_and_missing_boundary_are_not_qualified() {
        let mut input = fake_input(FakeDiagnosticScenario::Supported);
        input.architecture = "x86_64".to_string();
        input.processes[0].modules = vec!["WeChatAppEx Framework".to_string()];
        let report = diagnose_at(input, 5);
        assert!(!report.supported);
        assert!(!report.capture_allowed);
        assert!(report.candidates.is_empty());
        assert!(report
            .issues
            .iter()
            .any(|item| item.code == DiagnosticIssueCode::UnsupportedArchitecture));
    }

    #[test]
    fn mismatched_version_and_missing_framework_are_reported() {
        let mut input = fake_input(FakeDiagnosticScenario::Supported);
        input.wechat_version = "4.2.0".to_string();
        input.processes[0].wechat_version = "4.2.0".to_string();
        input.processes[0].modules = vec!["flue.dylib".to_string()];
        let report = diagnose_at(input, 6);
        assert_eq!(report.mode, DiagnosticMode::CompatibilityDiagnostic);
        assert!(!report.capture_allowed);
        assert!(report
            .issues
            .iter()
            .any(|item| item.code == DiagnosticIssueCode::MismatchedWechatVersion));
        // 版本不匹配时不会把模块看起来正确的进程误列为候选；模块原因仍
        // 通过统一的诊断结果由后续诊断清单覆盖。
        assert!(report.candidates.is_empty());
    }
}
