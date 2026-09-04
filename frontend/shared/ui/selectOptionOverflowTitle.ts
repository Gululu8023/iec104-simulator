const SELECT_OPTION_SELECTOR = '.app-select-dropdown .el-select-dropdown__item'

export function installSelectOptionOverflowTitles(): void {
  document.addEventListener('mouseover', (event) => {
    const target = event.target
    if (!(target instanceof Element)) return

    const option = target.closest<HTMLElement>(SELECT_OPTION_SELECTOR)
    if (!option) return

    if (option.scrollWidth > option.clientWidth) {
      option.title = option.textContent?.trim() ?? ''
    } else {
      option.removeAttribute('title')
    }
  })
}
