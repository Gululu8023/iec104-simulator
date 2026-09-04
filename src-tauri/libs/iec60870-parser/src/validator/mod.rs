//! 帧语义校验：`FrameRule` trait 让解析结果在零拷贝视图上执行额外检查。
//!
//! `FrameValidator` 默认组合了 VSQ/COT/质量/命令/参数/文件/安全 等规则。
//! 其中部分规则依赖 `validator::semantics` helper，部分规则会按需调用对应 parser
//! 进行语义检查。调用方可按需 `add_rule` 注入自定义逻辑。

pub mod rules;
pub mod semantics;

pub use rules::*;
use smallvec::SmallVec;

use crate::{error::ValidationError, types::FrameView};

/// 帧级别校验规则。
pub trait FrameRule {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError>;
}

/// 默认校验器：可组合多条规则。
#[derive(Default)]
pub struct FrameValidator {
    rules: SmallVec<[Box<dyn FrameRule + Send + Sync>; 4]>,
}

impl FrameValidator {
    pub fn new() -> Self {
        Self { rules: SmallVec::new() }
    }

    /// 提供内置规则：VSQ 非 0、COT 合法。
    pub fn with_builtin_rules() -> Self {
        let mut validator = Self::new();
        validator.add_rule(VsqRule);
        validator.add_rule(CauseRule);
        validator.add_rule(QualityRule::default());
        validator.add_rule(CommandRule);
        validator.add_rule(CommandCotRule);
        validator.add_rule(ParameterQualifierRule);
        validator.add_rule(FileTransferRule);
        validator.add_rule(SecuritySegmentRule);
        validator
    }

    pub fn add_rule<R>(&mut self, rule: R)
    where R: FrameRule + Send + Sync + 'static {
        self.rules.push(Box::new(rule));
    }
}

impl FrameRule for FrameValidator {
    fn validate(&self, frame: &FrameView) -> Result<(), ValidationError> {
        for rule in &self.rules {
            rule.validate(frame)?;
        }
        Ok(())
    }
}
