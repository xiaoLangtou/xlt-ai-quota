# 首次同步与同步响应设计

## 目标

应用首次打开后立即同步本机可用数据。每个连接器完成后立即刷新看板，而不是等待所有平台结束。OpenCode Go 的额度卡片暂不展示。

## 已确认范围

- 首次载入时自动触发一次同步。
- 手动同步仍可用；同步中的重复点击不发起第二次请求。
- 连接器成功或失败后，界面立即读取已持久化的部分结果。
- Ark Token 查询保留现有的降级策略，但每次尝试使用更短的专用超时，防止单个平台长时间拖住整体完成状态。
- 额度区域不再渲染 OpenCode Go 卡片；现有本地 Token 数据采集不在本次改动中扩展为订阅额度采集。

## 数据流

`useUsageDashboard()` 初始化后启动一次 `sync()`。`sync()` 将同步状态写入存储，再调用 `UsageService.syncAll(onProgress)`。每个 connector 完成时，`UsageService` 先写入数据或错误，再调用 `onProgress`；composable 在回调中执行 `readAll()`，使已完成的平台立即出现在卡片和图表中。所有任务结束后，服务统一写入最终同步状态和聚合错误。

## 超时策略

Vite 本地中间件保留通用 CLI 超时，但为 Ark 套餐与 Token 查询传入专用的较短超时。Ark Token 查询仍按当前 endpoint、API key、账户范围的顺序降级；每一步超时后记录错误并继续下一步。其他连接器不会因 Ark 的慢响应而延迟已完成数据的界面展示。

## 验证

新增脚本级冒烟测试，验证首次使用 composable 会自动请求同步、`UsageService` 在每个 connector 结束时产生进度通知、OpenCode Go 不再产生额度视图。保留既有聚合与设置测试，并运行 TypeScript 类型检查。