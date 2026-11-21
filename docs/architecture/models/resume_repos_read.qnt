// The repos-read count of a scan that pauses and is resumed (option A:
// the resuming run takes over the paused run's whole frozen plan, each row
// with the status it already holds).
//
// stored    — the LATEST run's plan rows, the only rows the queue counts.
// harvested — ghost: positions GitHub actually served in this scan.
// saved     — ghost: positions whose read the store accepted at some point.
module resume_repos_read {
  pure val Positions = Set(0, 1, 2)

  var stored: int -> str
  var harvested: Set[int]
  var saved: Set[int]
  var running: bool
  var outcome: str

  pure def readSetOf(rows: int -> str): Set[int] =
    rows.keys().filter(p => rows.get(p) == "read")

  pure def plannedOf(rows: int -> str): Set[int] =
    rows.keys().filter(p => rows.get(p) == "planned")

  // The resuming run's own plan: the whole frozen plan, statuses carried.
  pure def takenOver(rows: int -> str): int -> str = rows

  val reposRead = readSetOf(stored).size()

  action readAndSave(p: int): bool = all {
    running,
    stored.get(p) == "planned",
    stored' = stored.set(p, "read"),
    harvested' = harvested.union(Set(p)),
    saved' = saved.union(Set(p)),
    running' = running,
    outcome' = outcome,
  }

  // The store refuses the mark: the row stays planned and the run ends
  // interrupted (fix-repos-read-truthful).
  action readUnsaved(p: int): bool = all {
    running,
    stored.get(p) == "planned",
    stored' = stored,
    harvested' = harvested.union(Set(p)),
    saved' = saved,
    running' = false,
    outcome' = "interrupted",
  }

  action pause: bool = all {
    running,
    plannedOf(stored).size() > 0,
    stored' = stored,
    harvested' = harvested,
    saved' = saved,
    running' = false,
    outcome' = "rate_limited",
  }

  action resumeRun: bool = all {
    not(running),
    outcome == "rate_limited" or outcome == "interrupted",
    plannedOf(stored).size() > 0,
    stored' = takenOver(stored),
    harvested' = harvested,
    saved' = saved,
    running' = true,
    outcome' = "running",
  }

  // A row whose source the owner removed meanwhile.
  action skipRow(p: int): bool = all {
    running,
    stored.get(p) == "planned",
    stored' = stored.set(p, "skipped"),
    harvested' = harvested,
    saved' = saved,
    running' = running,
    outcome' = outcome,
  }

  action complete: bool = all {
    running,
    plannedOf(stored).size() == 0,
    stored' = stored,
    harvested' = harvested,
    saved' = saved,
    running' = false,
    outcome' = "completed",
  }

  action stutter: bool = all {
    outcome == "completed",
    stored' = stored,
    harvested' = harvested,
    saved' = saved,
    running' = running,
    outcome' = outcome,
  }

  action init: bool = all {
    stored' = Positions.mapBy(p => "planned"),
    harvested' = Set(),
    saved' = Set(),
    running' = true,
    outcome' = "running",
  }

  action step: bool = any {
    nondet p = Positions.oneOf(); readAndSave(p),
    nondet p = Positions.oneOf(); readUnsaved(p),
    nondet p = Positions.oneOf(); skipRow(p),
    pause,
    resumeRun,
    complete,
    stutter,
  }

  // P1: a read the store saved is still counted after any resume.
  val everySavedReadCounted = saved.subseteq(readSetOf(stored))
  // P2: the count names only rows whose read was saved — never twice,
  // never a row nobody read.
  val everyCountedReadSaved = readSetOf(stored).subseteq(saved)
  val countsOnlyHarvested = readSetOf(stored).subseteq(harvested)
  val countNeverExceedsPlan = reposRead <= Positions.size()
}
