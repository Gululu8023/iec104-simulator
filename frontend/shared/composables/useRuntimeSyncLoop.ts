import { listen, type UnlistenFn } from '@tauri-apps/api/event'

type UseRuntimeSyncLoopOptions<Task extends string, Payload> = {
  eventName: string
  intervalMs: number
  buildTaskSet: (tasks: Task[]) => Set<Task>
  shouldHandleEvent: (payload: Payload) => boolean
  resolveEventTasks: (payload: Payload) => Task[]
  resolvePollTasks: (tick: number) => Task[]
  runSync: (task: Task, isCurrent: () => boolean) => Promise<void>
  onEvent?: (payload: Payload) => void
  onListenError?: (error: unknown) => void
  onSyncError?: (error: unknown) => void
}

export function useRuntimeSyncLoop<Task extends string, Payload>(
  options: UseRuntimeSyncLoopOptions<Task, Payload>,
) {
  let runtimeHintUnlisten: UnlistenFn | null = null
  let syncTimer: ReturnType<typeof setInterval> | null = null
  const inFlightTasks = new Set<Task>()
  const dirtyTasks = new Set<Task>()
  let syncTick = 0
  let lifecycleRevision = 0
  let scopeRevision = 0
  let startPromise: Promise<void> | null = null

  const runTask = async (task: Task): Promise<void> => {
    const revision = scopeRevision
    if (inFlightTasks.has(task)) {
      dirtyTasks.add(task)
      return
    }

    inFlightTasks.add(task)
    try {
      await options.runSync(task, () => revision === scopeRevision)
    } catch (error) {
      if (revision === scopeRevision) options.onSyncError?.(error)
    } finally {
      if (revision !== scopeRevision) return
      inFlightTasks.delete(task)
      if (dirtyTasks.delete(task)) {
        void runTask(task)
      }
    }
  }

  const run = async (tasks: Task[]) => {
    const requestedTasks = options.buildTaskSet(tasks)
    await Promise.all([...requestedTasks].map((task) => runTask(task)))
  }

  const start = (): Promise<void> => {
    if (runtimeHintUnlisten || syncTimer) return Promise.resolve()
    if (startPromise) return startPromise
    const revision = ++lifecycleRevision
    scopeRevision += 1

    let promise: Promise<void>
    promise = (async () => {
      let unlisten: UnlistenFn | null = null
      try {
        unlisten = await listen<Payload>(options.eventName, (event) => {
          if (revision !== lifecycleRevision) return
          const payload = event.payload
          if (!payload || !options.shouldHandleEvent(payload)) return
          options.onEvent?.(payload)
          void run(options.resolveEventTasks(payload))
        })
      } catch (error) {
        if (revision === lifecycleRevision) options.onListenError?.(error)
      }

      if (revision !== lifecycleRevision) {
        unlisten?.()
        return
      }

      runtimeHintUnlisten = unlisten
      syncTick = 0
      syncTimer = setInterval(() => {
        if (revision !== lifecycleRevision) return
        const nextTasks = options.resolvePollTasks(syncTick)
        syncTick += 1
        void run(nextTasks)
      }, options.intervalMs)
    })().finally(() => {
      if (startPromise === promise) startPromise = null
    })
    startPromise = promise
    return promise
  }

  const stop = () => {
    lifecycleRevision += 1
    scopeRevision += 1
    startPromise = null
    if (runtimeHintUnlisten) {
      runtimeHintUnlisten()
      runtimeHintUnlisten = null
    }
    if (syncTimer) {
      clearInterval(syncTimer)
      syncTimer = null
    }
    inFlightTasks.clear()
    dirtyTasks.clear()
    syncTick = 0
  }

  const invalidate = () => {
    scopeRevision += 1
    inFlightTasks.clear()
    dirtyTasks.clear()
  }

  return {
    run,
    start,
    stop,
    invalidate,
  }
}
