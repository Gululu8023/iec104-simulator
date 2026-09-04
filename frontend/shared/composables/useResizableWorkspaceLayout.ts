import { ref } from 'vue'

export interface ResizableWorkspaceLayoutOptions {
  initialLeftWidth?: number
  minLeftWidth?: number
  maxLeftWidth?: number
  initialMainHeight?: number
  minMainHeight?: number
  maxMainHeight?: number
}

export function useResizableWorkspaceLayout(options: ResizableWorkspaceLayoutOptions = {}) {
  const leftPanelWidth = ref(options.initialLeftWidth ?? 280)
  const minLeftWidth = options.minLeftWidth ?? 280
  const maxLeftWidth = options.maxLeftWidth ?? 400
  const mainPanelsHeight = ref(options.initialMainHeight ?? 480)
  const minMainHeight = options.minMainHeight ?? 300
  const maxMainHeight = options.maxMainHeight ?? 700

  const beginResize = (cursor: 'ew-resize' | 'ns-resize', onMove: (event: MouseEvent) => void) => {
    const handleMouseUp = () => {
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', handleMouseUp)
      document.body.style.cursor = ''
      document.body.style.userSelect = ''
    }
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', handleMouseUp)
    document.body.style.cursor = cursor
    document.body.style.userSelect = 'none'
  }

  const startResizeLeft = (event: MouseEvent) => {
    const startX = event.clientX
    const startWidth = leftPanelWidth.value
    beginResize('ew-resize', (moveEvent) => {
      leftPanelWidth.value = Math.max(
        minLeftWidth,
        Math.min(maxLeftWidth, startWidth + moveEvent.clientX - startX),
      )
    })
  }

  const startResizeVertical = (event: MouseEvent) => {
    const startY = event.clientY
    const startHeight = mainPanelsHeight.value
    beginResize('ns-resize', (moveEvent) => {
      mainPanelsHeight.value = Math.max(
        minMainHeight,
        Math.min(maxMainHeight, startHeight + moveEvent.clientY - startY),
      )
    })
  }

  return { leftPanelWidth, mainPanelsHeight, startResizeLeft, startResizeVertical }
}
