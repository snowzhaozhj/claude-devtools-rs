## ADDED Requirements

### Requirement: 冷启共享项目数据

Sidebar 与 Dashboard 在应用冷启期间 SHALL 复用同一份前端 project/repositoryGroups 启动数据源，避免为同一批项目发现信息并行发起重复 IPC。实现 MUST 保持 session loading 边界：共享数据只覆盖项目 / repository group 摘要，不预加载所有项目 sessions。

#### Scenario: Dashboard 复用 Sidebar 的 repository groups 请求

- **WHEN** 应用冷启且无 active tab，Sidebar 与 Dashboard 同时渲染
- **THEN** 前端 SHALL 至多发起一次项目发现 IPC 请求用于获取 repository groups / projects
- **AND** Dashboard SHALL 从共享结果派生项目卡片
- **AND** Dashboard SHALL NOT 额外调用 `listProjects` 获取同一批冷启项目数据

#### Scenario: 共享项目数据不触发所有项目 sessions 加载

- **WHEN** Dashboard 从共享 project/repositoryGroups 数据渲染项目概览
- **THEN** Dashboard SHALL NOT 为每个项目调用 `listSessions`
- **AND** Sidebar SHALL 仍只为当前 `selectedProjectId` 请求第一页 sessions

#### Scenario: 共享请求失败时组件独立展示错误

- **WHEN** 冷启项目发现 IPC 失败
- **THEN** Sidebar 和 Dashboard SHALL 复用同一个失败结果
- **AND** 两个组件 MAY 按各自 UI 展示 loading/error 状态
- **AND** 前端 SHALL NOT 因两个组件同时等待而重复发起同一冷启项目发现请求
