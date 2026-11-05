//! Prequel program. Phase 0 stub: it exists only to prove the toolchain, IDL generation and the
//! LiteSVM test pipeline. `ping` is deleted when Phase 1 adds the real instructions (SPEC §5).

pub mod canon;
pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("FA6eApbY1Qqghuxo9gpRE8tpGWgsajQ6cWkotaKTrwEQ");

#[program]
pub mod prequel {
    use super::*;

    /// Toolchain probe: logs and succeeds.
    pub fn ping(ctx: Context<Ping>) -> Result<()> {
        crate::instructions::ping::handle_ping(ctx)
    }
}
