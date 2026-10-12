import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import type { Environment, FavoriteMode, Recording, Snapshot } from './types'

export const isDesktop = '__TAURI_INTERNALS__' in window
export let demoEnabled = false
const empty: Snapshot = { recordings: [], sources: [], categories: [], settings: { ffmpegPath: '', autoScan: true, continuousPlay: true }, diagnostics: [] }
let demo: Snapshot = structuredClone(empty)
const cloneDemo = () => structuredClone(demo)
export function enableDemo(): Snapshot {
  demoEnabled = true
  const titles = ['亚海悬城 · 关键回合', '霓虹町 · 三连击', '源工重镇 · 精彩瞬间', '隐世修所 · 防守回合', '莲华古城 · 极限反击', '日落之城 · 终局时刻']
  demo = { ...structuredClone(empty), categories: [{ id: 'aces', name: '高光时刻' }, { id: 'practice', name: '战术复盘' }], sources: [{ id: 'demo-local', name: '本机账号 · 演示', path: '演示数据 / 无本地文件', kind: 'wegame', account: 'VALORANT#001', status: '演示数据' }, { id: 'demo-import', name: '外部导入 · 演示', path: '演示数据 / 无本地文件', kind: 'import', account: 'Jett#002', status: '演示数据' }], diagnostics: ['当前为浏览器演示；录像与事件为样例，磁盘操作只在桌面端执行。'], recordings: titles.map((title, i): Recording => ({ id: `demo-${i}`, title, path: 'https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4', sourceId: i > 3 ? 'demo-import' : 'demo-local', account: i > 3 ? 'Jett#002' : 'VALORANT#001', createdAt: Date.now() / 1000 - i * 3700, duration: 5.055, width: 960, height: 540, fps: 30, size: 1128375, available: true, favoriteMode: i === 0 ? 'index' : 'none', favoritePath: null, categoryIds: i === 0 ? ['aces'] : [], metadataStatus: '演示样例事件', thumbnailPath: null, events: [{ id: `event-${i}-1`, kind: 'kill', time: 1.2, label: '击杀 · 样例' }, { id: `event-${i}-2`, kind: 'kill', time: 3.3, label: '击杀 · 样例' }] })) }
  return cloneDemo()
}
export async function command<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  if (isDesktop) return invoke<T>(name, args)
  if (!demoEnabled && name === 'get_library') return structuredClone(empty) as T
  if (name === 'get_environment') return { ffmpegAvailable: false, ffmpegPath: '', dataDir: '桌面端本地应用目录' } as T
  if (!demoEnabled) throw new Error('请通过 Tauri 启动桌面应用以访问本机录像。')
  const ids = args.ids as string[] | undefined
  switch (name) {
    case 'get_library': case 'scan_sources': return cloneDemo() as T
    case 'set_favorites': {
      if (args.mode === 'copy') throw new Error('永久副本需要桌面端访问本地文件；浏览器演示仅支持索引收藏。')
      demo.recordings.forEach(r => { if (ids?.includes(r.id)) r.favoriteMode = args.mode as FavoriteMode }); return cloneDemo() as T
    }
    case 'create_category': demo.categories.push({ id: `demo-cat-${Date.now()}`, name: String(args.name) }); return cloneDemo() as T
    case 'delete_category': demo.categories = demo.categories.filter(c => c.id !== args.id); demo.recordings.forEach(r => r.categoryIds = r.categoryIds.filter(id => id !== args.id)); return cloneDemo() as T
    case 'set_categories': demo.recordings.forEach(r => { if (ids?.includes(r.id)) r.categoryIds = [...args.categoryIds as string[]] }); return cloneDemo() as T
    case 'update_settings': demo.settings = args.settings as Snapshot['settings']; return cloneDemo() as T
    default: throw new Error('此操作需要桌面端访问本地文件。浏览器演示不会更改磁盘或导出样例。')
  }
}
export async function getEnvironment(): Promise<Environment> { return command('get_environment') }
export function mediaUrl(path: string | null): string { if (!path) return ''; return isDesktop ? convertFileSrc(path) : path }
export async function pickFolder(title: string): Promise<string | null> { if (!isDesktop) throw new Error('文件夹选择需要桌面应用。'); const result = await open({ title, directory: true, multiple: false }); return typeof result === 'string' ? result : null }
export async function pickVideos(): Promise<string[]> { if (!isDesktop) throw new Error('导入文件需要桌面应用。'); const result = await open({ title: '导入录像视频', multiple: true, filters: [{ name: '视频', extensions: ['mp4', 'mkv', 'mov', 'webm', 'avi', 'm4v'] }] }); return result ? typeof result === 'string' ? [result] : result : [] }
export async function pickMetadata(): Promise<string | null> { if (!isDesktop) throw new Error('导入元数据需要桌面应用。'); const result = await open({ title: '选择 ACLOS 元数据（包括无扩展名账号缓存）', multiple: false }); return typeof result === 'string' ? result : null }
export async function saveVideo(defaultPath: string): Promise<string | null> { if (!isDesktop) throw new Error('导出视频需要桌面应用。'); return save({ title: '导出 MP4 片段', defaultPath, filters: [{ name: 'MP4 视频', extensions: ['mp4'] }] }) }
export async function listenLibrary(callback: () => void, status: (message: string) => void): Promise<() => void> {
  if (!isDesktop) return () => {}
  const stops = await Promise.all([listen('library-updated', callback), listen<string>('scan-status', event => status(event.payload)), listen<{ message: string }>('operation-progress', event => status(event.payload.message))])
  return () => stops.forEach(stop => stop())
}
