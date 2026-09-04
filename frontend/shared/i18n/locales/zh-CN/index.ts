import common from './common'
import errors from './errors'
import settings from './settings'
import message from './message'
import protocol from './protocol'
import master from './master'
import slave from './slave'
import fileTransfer from './file-transfer'

const locale = {
  ...common,
  ...errors,
  ...settings,
  ...message,
  ...protocol,
  ...master,
  ...slave,
  master: {
    ...master.master,
    ...fileTransfer.master,
  },
}

export default locale
