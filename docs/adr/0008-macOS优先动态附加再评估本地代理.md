# macOS 优先动态附加再评估本地代理

macOS 首选通过 Frida 在 Flue/XWeb 的 HTTP 与 WebSocket 明文边界被动采集；如果 Hardened Runtime 阻止标准用户态附加，再评估需要明确授权安装本地 CA 的 HTTPS 代理后端。MiniTrace 不绕过证书固定或系统保护，两个路径都失败时将当前环境标记为不支持，而不是退回只保存 TLS 密文。
