# app-chrome Spec Delta

## MODIFIED Requirements

### Requirement: chrome 四 zone 布局

应用层 chrome 内部 SHALL 按 `[zone-platform-padding] [zone-left-center] [zone-drag-flex] [zone-status]` 四段 flex 横向布局：

- `zone-platform-padding`：仅 macOS 渲染，宽度 80 px，用于避让系统 traffic-light 按钮
- `zone-left-center`：放置主导航控件（项目选择下拉 + sidebar 折叠按钮），左对齐
- `zone-drag-flex`：flex: 1 弹性空白区，承载 `data-tauri-drag-region` 拖窗
- `zone-status`：右对齐的 status 容器，承载 status pill / status icon / notification button / settings button

平台判定 SHALL 以"运行平台为 macOS"为单一布尔信号驱动 padding 渲染；具体 detection 实现（userAgent / Tauri runtime API / 其它）属实现细节，spec 不绑死。

#### Scenario: macOS 平台 chrome 起始 padding

- **WHEN** 平台判定为 macOS
- **THEN** `zone-platform-padding` SHALL 渲染，宽度 SHALL 为 80 px
- **AND** `zone-left-center` 的第一个控件左边缘 SHALL 距窗口左边缘 80 px

#### Scenario: Windows / Linux 平台 chrome 起始 padding

- **WHEN** 平台判定为 Windows 或 Linux
- **THEN** `zone-platform-padding` SHALL NOT 渲染
- **AND** `zone-left-center` 的第一个控件左边缘 SHALL 距窗口左边缘 ≤ 8 px（仅保留 chrome 内边距）

#### Scenario: 拖动 chrome 非按钮区域移窗

- **WHEN** 用户在 chrome 的非按钮区域按住鼠标左键拖动
- **THEN** Tauri SHALL 调用窗口拖动（基于 `data-tauri-drag-region` 属性）
- **AND** 在按钮 / 下拉 / pill 上按下 SHALL NOT 触发拖窗（由 `data-tauri-drag-region="false"` 子树覆盖）
