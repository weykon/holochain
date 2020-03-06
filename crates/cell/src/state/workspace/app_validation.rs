use super::Workspace;
use crate::state::source_chain::SourceChainBuffer;
use sx_state::{db::DbManager, error::WorkspaceResult, prelude::*};

pub struct AppValidationWorkspace {

}

impl<'env> AppValidationWorkspace {
    pub fn new(reader: Reader<'env>, dbs: DbManager<'env>) -> WorkspaceResult<Self> {
        unimplemented!()
    }
}

impl<'env> Workspace for AppValidationWorkspace {
    fn commit_txn(self, _writer: Writer) -> WorkspaceResult<()> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod tests {

    use super::AppValidationWorkspace;
    use crate::state::source_chain::{SourceChainBuffer, SourceChainResult};
    use sx_state::{
        env::ReadManager, error::WorkspaceError, prelude::Readable, test_utils::test_env,
    };

    type Err = WorkspaceError;

    #[test]
    fn can_commit_workspace() -> SourceChainResult<()> {
        let arc = test_env();
        let env = arc.env();
        let dbs = arc.dbs()?;
        env.with_reader::<Err, _, _>(|reader| {
            let workspace = AppValidationWorkspace::new(reader, dbs);
            Ok(())
        })?;
        Ok(())
    }
}