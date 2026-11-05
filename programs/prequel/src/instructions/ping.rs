use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Ping<'info> {
    pub signer: Signer<'info>,
}

pub fn handle_ping(_ctx: Context<Ping>) -> Result<()> {
    msg!("prequel: pong");
    Ok(())
}
