## MODIFIED Requirements

### Requirement: General Section 展示

General section SHALL 展示当前配置值。MVP 阶段 SHALL 至少展示 theme 设置与数据根目录设置。数据根目录设置 SHALL 显示当前 `general.claudeRootPath`；当值为 `null` 时，UI SHALL 明确展示正在使用默认数据根目录。数据根目录的展示文案 SHALL 使用中性表述（“数据根目录”），不硬绑定特定来源名称。

数据根目录设置 SHALL 额外提供一个快速切换控件，其候选来自 `general.recentRoots`（详 [[configuration-management]]）：用户 SHALL 能从该控件一键切换到历史用过的数据根，无需重新手输或重新经文件选择器选目录。快速切换 SHALL 通过既有 `claudeRootPath` 更新路径落地，切换后新的当前值 SHALL 反映在展示控件中。

#### Scenario: 展示当前 theme
- **WHEN** General section 渲染
- **THEN** SHALL 显示当前 theme 值（dark/light/system）

#### Scenario: 展示默认数据根目录
- **WHEN** General section 渲染且 `general.claudeRootPath = null`
- **THEN** SHALL 显示“使用默认数据目录”或等价提示
- **AND** SHALL 显示默认 root 说明，帮助用户理解项目来自默认 `.claude/projects`

#### Scenario: 展示自定义数据根目录
- **WHEN** General section 渲染且 `general.claudeRootPath = "/data/claude-alt"`
- **THEN** SHALL 在输入框或等价控件中显示 `/data/claude-alt`

#### Scenario: 从历史快速切换数据根
- **WHEN** `general.recentRoots` 含至少一个历史路径，用户在快速切换控件中选择其中一项
- **THEN** 系统 SHALL 通过 `claudeRootPath` 更新路径切换到该数据根
- **AND** 切换后展示控件 SHALL 反映新的当前数据根

#### Scenario: 无历史时不阻塞手动输入
- **WHEN** `general.recentRoots` 为空
- **THEN** General section SHALL 仍支持手动输入路径与文件选择器选目录两条既有入口
