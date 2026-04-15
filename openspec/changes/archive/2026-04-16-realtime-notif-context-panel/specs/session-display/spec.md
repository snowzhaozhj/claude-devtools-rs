# session-display Specification (Delta — realtime-notif-context-panel)

> Delta spec：新增 Context Panel 增强 Requirements。

## ADDED Requirements

### Requirement: Context Panel 视图模式

Context Panel SHALL 支持 Category（按类别分组）和 Ranked（按 token 排序）两种视图模式。

#### Scenario: 默认 Category 视图
- **WHEN** Context Panel 打开
- **THEN** SHALL 默认显示 Category 视图，按类别分组展示注入项

#### Scenario: 切换到 Ranked 视图
- **WHEN** 用户点击 "Ranked" 按钮
- **THEN** SHALL 将所有注入项按 estimatedTokens 降序排列，平铺显示，每项带分类颜色标签

#### Scenario: 分类颜色系统
- **WHEN** Ranked 视图中渲染注入项
- **THEN** 各类别 SHALL 使用对应颜色标签：claude-md 紫蓝、file 绿、tool 黄、thinking 紫、team 橙、user 蓝

### Requirement: CLAUDE.md DirectoryTree

Category 视图中的 CLAUDE.md 类别 SHALL 以递归目录树形式展示文件路径。

#### Scenario: 目录树渲染
- **WHEN** CLAUDE.md 类别下有多个文件
- **THEN** SHALL 构建目录树，按路径层级递归渲染，目录可折叠/展开

#### Scenario: 文件节点信息
- **WHEN** 目录树中的文件节点渲染
- **THEN** SHALL 显示文件名和估计 token 数

#### Scenario: 目录排序
- **WHEN** 同级目录和文件排列
- **THEN** 文件 SHALL 排在目录之前，同类按名称字母排序
