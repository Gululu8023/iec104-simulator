type TreeNodeLike = {
  expanded?: boolean
  expand?: () => void
}

type TreeInstanceLike =
  | {
      getNode?: (key: string) => TreeNodeLike | null | undefined
    }
  | null
  | undefined

const normalizeKey = (value: unknown): string => String(value ?? '').trim()

export const appendExpandedTreeKey = (keys: string[], rawKey: unknown): string[] => {
  const key = normalizeKey(rawKey)
  if (!key || keys.includes(key)) {
    return keys
  }
  return [...keys, key]
}

export const removeExpandedTreeKey = (keys: string[], rawKey: unknown): string[] => {
  const key = normalizeKey(rawKey)
  if (!key) return keys
  return keys.filter((item) => item !== key)
}

export const syncExpandedTreeKeys = (
  keys: string[],
  validKeys: Iterable<string>,
  options: {
    fallbackKey?: string
    applyFallbackWhenEmpty?: boolean
  } = {},
): string[] => {
  const validSet = new Set(
    Array.from(validKeys)
      .map((item) => normalizeKey(item))
      .filter(Boolean),
  )
  const filtered = keys.filter((key) => validSet.has(normalizeKey(key)))

  if (filtered.length > 0 || !options.applyFallbackWhenEmpty) {
    return filtered
  }

  const fallbackKey = normalizeKey(options.fallbackKey)
  if (fallbackKey && validSet.has(fallbackKey)) {
    return [fallbackKey]
  }
  return filtered
}

export const replayExpandedTreeKeys = (tree: TreeInstanceLike, keys: string[]) => {
  if (!tree?.getNode) return
  for (const key of keys) {
    const node = tree.getNode(key)
    if (node && !node.expanded) {
      node.expand?.()
    }
  }
}

/**
 * 通用展开事件处理：仅对指定类型的节点追踪展开 key。
 * 用于 el-tree 的 @node-expand 回调。
 */
export const handleNodeExpandByTypes = (
  data: { type?: string; id?: unknown },
  expandedKeys: string[],
  allowedTypes: string[],
): string[] => {
  if (!allowedTypes.includes(data.type ?? '')) return expandedKeys
  return appendExpandedTreeKey(expandedKeys, data.id)
}

/**
 * 通用折叠事件处理：仅对指定类型的节点移除展开 key。
 * 用于 el-tree 的 @node-collapse 回调。
 */
export const handleNodeCollapseByTypes = (
  data: { type?: string; id?: unknown },
  expandedKeys: string[],
  allowedTypes: string[],
): string[] => {
  if (!allowedTypes.includes(data.type ?? '')) return expandedKeys
  return removeExpandedTreeKey(expandedKeys, data.id)
}

/**
 * 处理树节点点击时的展开/折叠逻辑：
 * - 切换节点（isSwitching=true）：确保展开（已展开不变，折叠则展开）
 * - 点击当前节点（isSwitching=false）：toggle 展开/折叠
 *
 * @returns 更新后的展开 keys 数组
 */
export const handleTreeNodeClickExpansion = (
  tree: TreeInstanceLike,
  nodeId: string,
  isSwitching: boolean,
  expandedKeys: string[],
): string[] => {
  if (!tree?.getNode) return expandedKeys
  const treeNode = tree.getNode(nodeId)
  if (!treeNode) return expandedKeys

  if (isSwitching) {
    // 切换节点：确保展开
    if (!treeNode.expanded) {
      treeNode.expanded = true
      return appendExpandedTreeKey(expandedKeys, nodeId)
    }
    return expandedKeys
  }

  // 点击当前节点：toggle
  treeNode.expanded = !treeNode.expanded
  if (treeNode.expanded) {
    return appendExpandedTreeKey(expandedKeys, nodeId)
  }
  return removeExpandedTreeKey(expandedKeys, nodeId)
}
