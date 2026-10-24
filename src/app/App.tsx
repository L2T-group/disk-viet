import { useEffect, useState } from 'react';
import { DeleteDialog, type DeleteDone } from '@/features/delete/DeleteDialog';
import { ResultDialog } from '@/features/delete/ResultDialog';
import { DetailsPanel } from '@/features/details/DetailsPanel';
import { SelectionBar } from '@/features/selection/SelectionBar';
import { reopenId } from '@/features/selection/select';
import { TopBar } from '@/features/topbar/TopBar';
import { Legend } from '@/features/treemap/Legend';
import { Treemap } from '@/features/treemap/Treemap';
import type { NodeView } from '@/lib/types';
import { useScan } from '@/stores/scan';

export function App() {
  useEffect(() => void useScan.getState().init(), []);
  const [toDelete, setToDelete] = useState<NodeView[] | null>(null);
  const [done, setDone] = useState<DeleteDone | null>(null);

  function onDone(d: DeleteDone) {
    setToDelete(null);
    setDone(d);
    const s = useScan.getState();
    s.clearSelection();
    const gone = d.report.results.filter((x) => x.ok).map((x) => x.path);
    if (s.level) void s.open(reopenId(s.level, gone));
  }

  return (
    <div className="flex h-full flex-col">
      <TopBar />
      <Legend />
      <div className="flex min-h-0 flex-1">
        <Treemap />
        <DetailsPanel onDelete={(n) => setToDelete([n])} />
      </div>
      <SelectionBar onDelete={setToDelete} />
      {toDelete && <DeleteDialog nodes={toDelete} onClose={() => setToDelete(null)} onDone={onDone} />}
      {done && <ResultDialog done={done} onClose={() => setDone(null)} />}
    </div>
  );
}
