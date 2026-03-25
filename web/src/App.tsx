import { useState, useMemo } from 'react'
import { useStats } from './useStats'
import type { SortKey, SortDir, StatEntry } from './types'
import './App.css'

// ---------------------------------------------------------------------------
// Bar chart
// ---------------------------------------------------------------------------

interface BarChartProps {
  entries: StatEntry[]
  maxBars?: number
}

function BarChart({ entries, maxBars = 15 }: BarChartProps) {
  const top = entries.slice(0, maxBars)
  const maxCount = top.reduce((m, e) => Math.max(m, e.count), 1)

  if (top.length === 0) {
    return <p className="empty-msg">No data yet.</p>
  }

  return (
    <div className="bar-chart">
      {top.map((e) => {
        const pct = Math.max(2, (e.count / maxCount) * 100)
        return (
          <div key={`${e.protocol}-${e.port}`} className="bar-row">
            <span className="bar-label">
              <span className={`proto-badge proto-${e.protocol}`}>{e.protocol}</span>
              {e.port}
            </span>
            <div className="bar-track">
              <div className="bar-fill" style={{ width: `${pct}%` }} />
            </div>
            <span className="bar-count">{e.count.toLocaleString()}</span>
          </div>
        )
      })}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Stats table
// ---------------------------------------------------------------------------

interface StatsTableProps {
  entries: StatEntry[]
  sortKey: SortKey
  sortDir: SortDir
  onSort: (key: SortKey) => void
}

function SortIndicator({ active, dir }: { active: boolean; dir: SortDir }) {
  if (!active) return <span className="sort-inactive"> ⇅</span>
  return <span className="sort-active"> {dir === 'asc' ? '↑' : '↓'}</span>
}

function StatsTable({ entries, sortKey, sortDir, onSort }: StatsTableProps) {
  if (entries.length === 0) {
    return <p className="empty-msg">Waiting for packets...</p>
  }

  return (
    <table className="stats-table">
      <thead>
        <tr>
          <th onClick={() => onSort('protocol')} className="sortable">
            Protocol<SortIndicator active={sortKey === 'protocol'} dir={sortDir} />
          </th>
          <th onClick={() => onSort('port')} className="sortable">
            Port<SortIndicator active={sortKey === 'port'} dir={sortDir} />
          </th>
          <th onClick={() => onSort('count')} className="sortable">
            Packets<SortIndicator active={sortKey === 'count'} dir={sortDir} />
          </th>
        </tr>
      </thead>
      <tbody>
        {entries.map((e) => (
          <tr key={`${e.protocol}-${e.port}`}>
            <td>
              <span className={`proto-badge proto-${e.protocol}`}>{e.protocol}</span>
            </td>
            <td className="num">{e.port}</td>
            <td className="num">{e.count.toLocaleString()}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

// ---------------------------------------------------------------------------
// Connection status badge
// ---------------------------------------------------------------------------

function StatusBadge({ status }: { status: string }) {
  const label =
    status === 'ok' ? 'Live' : status === 'error' ? 'Disconnected' : 'Connecting...'
  return <span className={`status-badge status-${status}`}>{label}</span>
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

export default function App() {
  const [autoRefresh, setAutoRefresh] = useState(true)
  const [sortKey, setSortKey] = useState<SortKey>('count')
  const [sortDir, setSortDir] = useState<SortDir>('desc')

  const { stats, status, lastUpdated, refresh } = useStats({
    intervalMs: 2000,
    enabled: autoRefresh,
  })

  const sorted = useMemo(() => {
    const copy = [...stats]
    copy.sort((a, b) => {
      let cmp = 0
      if (sortKey === 'count') cmp = a.count - b.count
      else if (sortKey === 'port') cmp = a.port - b.port
      else cmp = a.protocol.localeCompare(b.protocol)
      return sortDir === 'asc' ? cmp : -cmp
    })
    return copy
  }, [stats, sortKey, sortDir])

  function handleSort(key: SortKey) {
    if (key === sortKey) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortKey(key)
      setSortDir('desc')
    }
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>Packet Counter</h1>
        <div className="header-controls">
          <StatusBadge status={status} />
          {lastUpdated && (
            <span className="last-updated">
              Updated {lastUpdated.toLocaleTimeString()}
            </span>
          )}
          <button
            type="button"
            className={`toggle-btn ${autoRefresh ? 'active' : ''}`}
            onClick={() => setAutoRefresh((v) => !v)}
          >
            {autoRefresh ? 'Pause' : 'Resume'}
          </button>
          <button type="button" className="refresh-btn" onClick={refresh} disabled={autoRefresh}>
            Refresh
          </button>
        </div>
      </header>

      <main className="app-main">
        <section className="section">
          <h2>Top Ports (by packets)</h2>
          <BarChart entries={sorted} maxBars={15} />
        </section>

        <section className="section">
          <h2>All Stats</h2>
          <StatsTable
            entries={sorted}
            sortKey={sortKey}
            sortDir={sortDir}
            onSort={handleSort}
          />
        </section>
      </main>
    </div>
  )
}
