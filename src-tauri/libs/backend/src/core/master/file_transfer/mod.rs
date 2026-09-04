//! 主站文件传输子模块。
//!
//! 本模块负责主站的文件传输功能,包括:
//!
//! - **传输运行时**:管理文件传输会话的生命周期
//! - **传输存储**:存储文件传输的状态和数据

pub(crate) mod runtime;
pub(crate) mod store;

pub(crate) use runtime::{
    MasterFileTransferRuntime, cancel_master_file_transfer, get_master_file_transfer_session,
    start_master_file_transfer,
};
