# 06 — 接入 Windows 采集后端

**What to build:** 将既有 Windows `flue.dll` Python/Frida 采集逻辑接入 MiniTrace 的统一后端契约，使同一桌面界面和采集会话模型能够驱动 Windows 后端。由于当前没有 Windows 实机，本 Ticket 只交付平台接入，不宣称平台采集能力。

**Blocked by:** 01 — 建立桌面端最小采集闭环

**Status:** ready-for-agent

- [ ] Windows 后端遵守与 macOS 相同的诊断、候选进程、开始、事件流、正常停止和异常结束契约。
- [ ] 既有 `flue.dll` 采集证据链和协议拆帧行为被复用，不因桌面外壳建设而重写或丢弃。
- [ ] 桌面界面不包含依赖 macOS 实现细节的流程或状态判断。
- [ ] Windows 后端通过统一契约的自动化测试，测试不依赖真实 Windows 微信实例。
- [ ] 在完成当前 Windows 微信版本真实流量验收前，界面固定显示“平台接入、未验证”。
- [ ] Windows 真实验收和正式自包含打包作为后续工作，不在本 Ticket 中虚报完成。
