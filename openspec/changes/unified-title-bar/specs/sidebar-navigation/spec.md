## MODIFIED Requirements

### Requirement: 项目选择

`UnifiedTitleBar` 的 `zone-left-center` SHALL 提供项目选择下拉作为主导航控件。选择项目后 SHALL 自动加载该项目的会话列表到 Sidebar。项目选择控件 MUST NOT 渲染在 `SidebarHeader.svelte` 或 Sidebar 内部，MUST NOT 随 sidebar 折叠状态消失。

#### Scenario: 初始加载
- **WHEN** 应用启动且有可用项目
- **THEN** 系统 SHALL 自动选中第一个项目并加载其会话列表
- **AND** chrome 内项目下拉 SHALL 显示当前选中项目的名称

#### Scenario: 切换项目
- **WHEN** 用户从 chrome 内项目下拉选择器切换到另一个项目
- **THEN** 会话列表 SHALL 更新为新项目的会话，之前的列表 SHALL 被替换
- **AND** chrome 内项目下拉 SHALL 显示新选中项目的名称

#### Scenario: 无项目
- **WHEN** 无可用项目
- **THEN** Sidebar SHALL 显示空状态提示
- **AND** chrome 内项目下拉 SHALL 显示禁用态占位文本（如「无项目」）

#### Scenario: sidebar 折叠不影响项目选择
- **WHEN** 用户点击 chrome 内 sidebar 折叠按钮把 sidebar 收起
- **THEN** chrome 内项目下拉 SHALL 仍可见且可操作
- **AND** 用户 SHALL 可在 sidebar 折叠态切换项目，新项目的会话列表会在重新展开 sidebar 时立即可见
