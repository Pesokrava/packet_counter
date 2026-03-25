import { useState, useEffect, useRef, useCallback } from 'react'
import type { StatEntry } from './types'

export type ConnectionStatus = 'connecting' | 'ok' | 'error'

interface UseStatsOptions {
  intervalMs?: number
  enabled?: boolean
}

interface UseStatsResult {
  stats: StatEntry[]
  status: ConnectionStatus
  lastUpdated: Date | null
  refresh: () => void
}

export function useStats({
  intervalMs = 2000,
  enabled = true,
}: UseStatsOptions = {}): UseStatsResult {
  const [stats, setStats] = useState<StatEntry[]>([])
  const [status, setStatus] = useState<ConnectionStatus>('connecting')
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null)
  const abortRef = useRef<AbortController | null>(null)

  const fetchStats = useCallback(async () => {
    abortRef.current?.abort()
    const ac = new AbortController()
    abortRef.current = ac

    try {
      const res = await fetch('/api/stats', { signal: ac.signal })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data: StatEntry[] = await res.json()
      setStats(data)
      setStatus('ok')
      setLastUpdated(new Date())
    } catch (err) {
      if ((err as Error).name === 'AbortError') return
      setStatus('error')
    }
  }, [])

  useEffect(() => {
    if (!enabled) return

    fetchStats()
    const id = setInterval(fetchStats, intervalMs)
    return () => {
      clearInterval(id)
      abortRef.current?.abort()
    }
  }, [enabled, intervalMs, fetchStats])

  return { stats, status, lastUpdated, refresh: fetchStats }
}
