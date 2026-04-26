/**
 * 自动更新前端状态机。
 *
 * 数据来源 = 路径 A：前端直接调 `@tauri-apps/plugin-updater` 的 JS API，
 * 进度回调直接写入 store；后端不中转 download progress event。
 *
 * 与后端 `updater://available` 事件配合：startup 检查在后端发送，
 * 前端 listen 后写入 store；`available` 状态触发 UpdateBanner 三按钮交互。
 *
 * 行为契约：openspec/specs/app-auto-update/spec.md
 */

import { invoke } from '@tauri-apps/api/core'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export type UpdateStatus =
  | 'idle'
  | 'available'
  | 'downloading'
  | 'downloaded'
  | 'error'

export interface UpdateAvailablePayload {
  currentVersion: string
  newVersion: string
  notes: string
  signatureOk: boolean
}

class UpdateStore {
  status = $state<UpdateStatus>('idle')
  currentVersion = $state<string>('')
  newVersion = $state<string>('')
  notes = $state<string>('')
  signatureOk = $state<boolean>(true)
  /** 已下载字节 / 总字节，仅 status === 'downloading' 时有效 */
  downloaded = $state<number>(0)
  contentLength = $state<number>(0)
  /** 下载/安装阶段的错误描述 */
  errorMessage = $state<string>('')
  /** banner 是否对用户可见。「稍后提醒」会把它置 false 但保留 status */
  visible = $state<boolean>(false)

  /** 当前下载所持有的 Update 对象（可选，用于取消） */
  private update: Update | null = null

  /**
   * 由 `updater://available` event 或手动检查的 `available` 结果驱动。
   * 不直接持有 Update 对象（event 来自后端）；下载时再调一次 `check()` 拿。
   */
  showAvailable(payload: UpdateAvailablePayload): void {
    this.status = 'available'
    this.currentVersion = payload.currentVersion
    this.newVersion = payload.newVersion
    this.notes = payload.notes
    this.signatureOk = payload.signatureOk
    this.errorMessage = ''
    this.visible = true
  }

  /**
   * 「稍后提醒」：关闭 banner 但保留 status；下次启动重新检查。
   * 当前会话内不再展示同版本。
   */
  remindLater(): void {
    this.visible = false
  }

  /**
   * 「跳过此版本」：写入 config，关闭 banner。
   */
  async skipVersion(): Promise<void> {
    const version = this.newVersion
    this.visible = false
    try {
      await invoke('update_config', {
        section: 'updater',
        configData: { skippedUpdateVersion: version },
      })
    } catch (e) {
      // 写失败时回滚 visible 让用户重试；store 不持久化错误
      this.visible = true
      throw e
    }
  }

  /**
   * 「立即更新」：再次 `check()` 拿 Update 对象后 `downloadAndInstall`。
   * Linux .deb 包在此处会抛错（plugin 限制），调用方捕获后弹「请到 GitHub 下载」。
   */
  async downloadAndInstall(): Promise<void> {
    this.status = 'downloading'
    this.downloaded = 0
    this.contentLength = 0
    this.errorMessage = ''

    try {
      const update = await check()
      if (!update) {
        // 极少见：startup 通知后服务器又把 release 撤了
        this.status = 'idle'
        this.visible = false
        return
      }
      this.update = update

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            this.contentLength = event.data.contentLength ?? 0
            break
          case 'Progress':
            this.downloaded += event.data.chunkLength
            break
          case 'Finished':
            this.status = 'downloaded'
            break
        }
      })

      // downloadAndInstall 完成后立即重启
      await relaunch()
    } catch (e) {
      this.status = 'error'
      this.errorMessage = e instanceof Error ? e.message : String(e)
      throw e
    } finally {
      this.update = null
    }
  }

  /**
   * 关闭 banner（X 按钮）。下载中需要 confirm，由 UI 层处理。
   */
  dismiss(): void {
    this.visible = false
    if (this.status !== 'downloading') {
      this.status = 'idle'
    }
  }

  /** 仅测试用。生产 store 是单例，状态自然累积。 */
  reset(): void {
    this.status = 'idle'
    this.currentVersion = ''
    this.newVersion = ''
    this.notes = ''
    this.signatureOk = true
    this.downloaded = 0
    this.contentLength = 0
    this.errorMessage = ''
    this.visible = false
    this.update = null
  }
}

export const updateStore = new UpdateStore()
