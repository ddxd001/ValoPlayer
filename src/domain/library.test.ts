import { describe, expect, it } from 'vitest'
import { clampClip, countKills, fileStem, filterRecordings, formatTime } from './library'
import type { KillCountFilter, LibraryView, MomentEvent, Recording } from '../types'

const item = (overrides: Partial<Recording>): Recording => ({ id: 'a', title: '亚海悬城', path: 'a.mp4', sourceId: 'local', account: 'player', createdAt: 1, duration: 15, width: 1920, height: 1080, fps: 60, size: 0, available: true, favoriteMode: 'none', favoritePath: null, categoryIds: [], events: [], metadataStatus: '', thumbnailPath: null, ...overrides })
const kills = (count: number, label = '防守击杀'): MomentEvent[] => Array.from({ length: count }, (_, index) => ({ id: `kill-${index}`, kind: 'kill', time: index + 1, label }))
describe('library filtering', () => {
  it('combines category, account and event searches and preserves original order', () => {
    const recordings = [item({ id: 'one', categoryIds: ['c'], events: [{ id: 'e', kind: 'kill', time: 2, label: '三连击' }] }), item({ id: 'two', account: 'other', categoryIds: ['c'], createdAt: 3 })]
    expect(filterRecordings(recordings, 'category:c', 'player', '三连击', 'newest').map(r => r.id)).toEqual(['one'])
    expect(recordings.map(r => r.id)).toEqual(['one', 'two'])
  })
  it('distinguishes imported sources and both favorite modes', () => {
    const recordings = [item({ id: 'a', favoriteMode: 'index' }), item({ id: 'b', favoriteMode: 'copy', sourceId: 'import' }), item({ id: 'c' })]
    expect(filterRecordings(recordings, 'favorites', '', '', 'newest')).toHaveLength(2)
    expect(filterRecordings(recordings, 'imports', '', '', 'newest', ['import']).map(r => r.id)).toEqual(['b'])
  })
})

describe('kill event count filtering', () => {
  it.each(['1', '2', '3', '4', '5'] as KillCountFilter[])('matches exactly %s kill events', filter => {
    const recordings = Array.from({ length: 8 }, (_, count) => item({ id: String(count), events: kills(count) }))
    expect(filterRecordings(recordings, 'all', '', '', 'newest', [], filter).map(r => r.id)).toEqual([filter])
  })
  it('groups six or more kill events without matching a five-kill recording', () => {
    const recordings = [5, 6, 7, 10].map(count => item({ id: String(count), events: kills(count) }))
    expect(filterRecordings(recordings, 'all', '', '', 'newest', [], '6+').map(r => r.id)).toEqual(['6', '7', '10'])
  })
  it('counts only kill events regardless of titles and other event labels', () => {
    const recording = item({ title: 'ACE · 五杀 · 6 连杀', events: [...kills(3), { id: 'death', kind: 'death', time: 4, label: '击杀' }, { id: 'assist', kind: 'assist', time: 5, label: '击杀' }, { id: 'round', kind: 'round', time: 6, label: '击杀' }, { id: 'unknown', kind: 'unknown', time: 7, label: 'kill' }] })
    expect(countKills(recording)).toBe(3)
    expect(filterRecordings([recording], 'all', '', '', 'newest', [], '3')).toHaveLength(1)
    expect(filterRecordings([recording], 'all', '', '', 'newest', [], '5')).toHaveLength(0)
  })
  it('zero includes no events and recordings containing only non-kill events', () => {
    const recordings = [item({ id: 'none', title: '三杀精彩时刻' }), item({ id: 'death-only', events: [{ id: 'e', kind: 'death', time: 1, label: '击杀' }] }), item({ id: 'one', events: kills(1) })]
    expect(filterRecordings(recordings, 'all', '', '', 'newest', [], '0').map(r => r.id)).toEqual(['none', 'death-only'])
    expect(filterRecordings(recordings, 'all', '', '', 'newest').map(r => r.id)).toEqual(['none', 'death-only', 'one'])
  })
  it.each([
    ['favorites', { favoriteMode: 'none' }],
    ['imports', { sourceId: 'other' }],
    ['category:c', { categoryIds: [] }],
    ['source:import', { sourceId: 'other' }],
  ] as [LibraryView, Partial<Recording>][])('combines %s with account, query, kills and sorting without mutating input', (view, excluded) => {
    const base: Partial<Recording> = { sourceId: 'import', favoriteMode: 'index', categoryIds: ['c'], events: kills(3) }
    const recordings = [
      item({ ...base, id: 'older', createdAt: 1 }),
      item({ ...base, id: 'newer', favoriteMode: 'copy', createdAt: 4 }),
      item({ ...base, id: 'other-account', account: 'other', createdAt: 8 }),
      item({ ...base, id: 'two-kills', events: kills(2), createdAt: 7 }),
      item({ ...base, id: 'other-query', events: kills(3, '进攻击杀'), createdAt: 6 }),
      item({ ...base, id: 'outside-view', ...excluded, createdAt: 5 }),
    ]
    const original = structuredClone(recordings)
    expect(filterRecordings(recordings, view, 'player', '防守', 'newest', ['import'], '3').map(r => r.id)).toEqual(['newer', 'older'])
    expect(filterRecordings(recordings, view, 'player', '防守', 'oldest', ['import'], '3').map(r => r.id)).toEqual(['older', 'newer'])
    expect(recordings).toEqual(original)
  })
})
describe('media boundaries', () => {
  it('formats hours and handles invalid durations', () => { expect(formatTime(3661.5)).toBe('1:01:01'); expect(formatTime(NaN)).toBe('00:00'); expect(formatTime(-8)).toBe('00:00') })
  it('clamps ranges without accepting reversed or empty clips', () => { expect(clampClip(-2, 12, 10)).toEqual({ start: 0, end: 10, valid: true }); expect(clampClip(5, 3, 10).valid).toBe(false); expect(clampClip(0, Infinity, 10).valid).toBe(false) })
  it('makes Windows-safe export names', () => { expect(fileStem('ACE: A/B?')).toBe('ACE_ A_B_'); expect(fileStem('...')).toBe('moment') })
})
