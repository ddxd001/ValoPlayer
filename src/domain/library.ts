import type { KillCountFilter, LibraryView, Recording, SortOrder } from '../types'

export function formatTime(seconds: number): string {
  const total = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0))
  const hours = Math.floor(total / 3600), minutes = Math.floor(total / 60) % 60
  return `${hours ? `${hours}:` : ''}${String(minutes).padStart(2, '0')}:${String(total % 60).padStart(2, '0')}`
}
export function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(0, bytes / 1024).toFixed(0)} KB`
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`
}
export function countKills(recording: Pick<Recording, 'events'>): number {
  return recording.events.reduce((count, event) => count + (event.kind === 'kill' ? 1 : 0), 0)
}
export function filterRecordings(recordings: Recording[], view: LibraryView, account: string, query: string, sort: SortOrder, importSourceIds: string[] = [], killFilter: KillCountFilter = 'all'): Recording[] {
  const search = query.trim().toLocaleLowerCase()
  return recordings.filter(r => {
    if (view === 'favorites' && r.favoriteMode === 'none') return false
    if (view === 'imports' && !importSourceIds.includes(r.sourceId)) return false
    if (view.startsWith('category:') && !r.categoryIds.includes(view.slice(9))) return false
    if (view.startsWith('source:') && r.sourceId !== view.slice(7)) return false
    if (account && r.account !== account) return false
    if (killFilter !== 'all') {
      const kills = countKills(r)
      if (killFilter === '6+' ? kills < 6 : kills !== Number(killFilter)) return false
    }
    return !search || `${r.title} ${r.account} ${r.events.map(e => e.label).join(' ')}`.toLocaleLowerCase().includes(search)
  }).sort((a, b) => sort === 'title' ? a.title.localeCompare(b.title, 'zh-CN') : sort === 'duration' ? b.duration - a.duration : sort === 'oldest' ? a.createdAt - b.createdAt : b.createdAt - a.createdAt)
}
export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return !!target.closest('input, textarea, select, [contenteditable="true"], [role="textbox"]')
}
export function clampClip(start: number, end: number, duration: number): { start: number; end: number; valid: boolean } {
  const safeStart = Math.max(0, Math.min(duration, start)), safeEnd = Math.max(0, Math.min(duration, end))
  return { start: safeStart, end: safeEnd, valid: Number.isFinite(start) && Number.isFinite(end) && safeEnd > safeStart && duration > 0 }
}
export function fileStem(name: string): string { return name.replace(/[<>:"/\\|?*]/g, '_').replace(/[. ]+$/, '') || 'moment' }
