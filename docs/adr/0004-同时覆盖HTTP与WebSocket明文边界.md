# 同时覆盖 HTTP 与 WebSocket 明文边界

macOS 平台采集能力必须覆盖《世界 Online》的 HTTP、HTTPS 和 WebSocket 明文，因此同时验证 Flue/XWeb 中的 HTTP 与 WebSocket 数据路径，不能只以 Node `TLSWrap` 或单一 WebSocket Hook 代表完整支持。多边界可能产生重复观察，后续必须保留来源并明确关联，而不是静默去重原始证据。
