<script lang="ts">
  let {
    headers = [] as string[],
    rows = [] as string[][],
    severityColumn = -1,
  }: {
    headers?: string[];
    rows?: string[][];
    severityColumn?: number;
  } = $props();

  function severityClass(text: string): string {
    const t = text.toUpperCase();
    if (t === 'CRITICAL') return 'severity-critical';
    if (t === 'HIGH') return 'severity-high';
    if (t === 'MEDIUM') return 'severity-medium';
    if (t === 'LOW') return 'severity-low';
    if (t === 'NOTE') return 'severity-note';
    if (t === 'OPTIMAL') return 'verdict-optimal';
    if (t.includes('NEAR-OPTIMAL')) return 'verdict-near';
    if (t === 'YES' || t === 'PASS') return 'verdict-pass';
    if (t === 'NO' || t === 'FAIL') return 'verdict-fail';
    if (t === 'NEW') return 'status-new';
    if (t === 'FIXED') return 'status-fixed';
    return '';
  }
</script>

<div class="data-table-wrapper">
  <table class="data-table">
    {#if headers.length > 0}
      <thead>
        <tr>
          {#each headers as header}
            <th>{header}</th>
          {/each}
        </tr>
      </thead>
    {/if}
    <tbody>
      {#each rows as row}
        <tr>
          {#each row as cell, ci}
            {@const badge = (ci === severityColumn || ci >= headers.length - 2) ? severityClass(cell) : ''}
            <td>
              {#if badge}
                <span class="badge {badge}">{cell}</span>
              {:else}
                {cell}
              {/if}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .data-table-wrapper {
    overflow-x: auto;
    border: 1px solid var(--color-border, #2a2b35);
    border-radius: var(--radius-md, 8px);
  }

  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8125rem;
  }

  th {
    padding: var(--space-2, 8px) var(--space-3, 12px);
    background: var(--color-bg-tertiary, #1a1b23);
    font-weight: 600;
    font-size: 0.6875rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #636874);
    text-align: left;
    border-bottom: 1px solid var(--color-border, #2a2b35);
    white-space: nowrap;
  }

  td {
    padding: var(--space-2, 8px) var(--space-3, 12px);
    border-bottom: 1px solid var(--color-border, #2a2b35);
    color: var(--color-text-secondary, #9ca3af);
    font-family: var(--font-mono, monospace);
  }

  tr:last-child td {
    border-bottom: none;
  }

  tr:hover td {
    background: rgba(86, 156, 214, 0.03);
  }

  .badge {
    display: inline-block;
    padding: 1px 8px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.625rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    white-space: nowrap;
  }

  .severity-critical { background: rgba(239, 68, 68, 0.15); color: #ef4444; }
  .severity-high     { background: rgba(249, 115, 22, 0.15); color: #f97316; }
  .severity-medium   { background: rgba(251, 191, 36, 0.15); color: #fbbf24; }
  .severity-low      { background: rgba(96, 165, 250, 0.15); color: #60a5fa; }
  .severity-note     { background: rgba(156, 163, 175, 0.1); color: #9ca3af; }

  .verdict-optimal   { background: rgba(74, 222, 128, 0.15); color: #4ade80; }
  .verdict-near      { background: rgba(251, 191, 36, 0.12); color: #fbbf24; }
  .verdict-pass      { background: rgba(74, 222, 128, 0.15); color: #4ade80; }
  .verdict-fail      { background: rgba(239, 68, 68, 0.15); color: #ef4444; }

  .status-new        { background: rgba(96, 165, 250, 0.12); color: #60a5fa; }
  .status-fixed      { background: rgba(74, 222, 128, 0.12); color: #4ade80; }
</style>
