import { invokeCommand } from './tauri'

import type { MessageFrameParseResult } from './types'

export async function parseMessageFrame(inputText: string): Promise<MessageFrameParseResult> {
  return invokeCommand<MessageFrameParseResult>('parse_message_frame', {
    request: {
      input_text: inputText,
    },
  })
}
