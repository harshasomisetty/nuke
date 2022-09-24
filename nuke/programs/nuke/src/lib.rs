use anchor_lang::prelude::*;
use solana_program::{
    account_info::next_account_info,
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    pubkey::Pubkey,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};

declare_id!("HzwyTmrungBwbSmaBPPRo97iTC8Grqv7AQw297BGFsF2");

#[program]
pub mod nuke {
    use super::*;

    pub fn spam(ctx: Context<Spam>, random: u32, loop_counter: u16, amount: u64) -> Result<()> {
        let bad_actor = &mut ctx.accounts.bad_actor;
        let receiver = &mut ctx.accounts.receiver;
        let system_program = &mut ctx.accounts.system_program;

        msg!("random data: {}, loop counter: {}", random, loop_counter);
        for n in (1..loop_counter).map(|n| n as u8) {
            let (map_pda, _) = Pubkey::find_program_address(
                &[b"spam".as_ref(), &n.to_be_bytes(), &random.to_be_bytes()],
                &id(),
            );
            msg!("{}", &map_pda.to_string());
        }

        invoke(
            &system_instruction::transfer(
                &bad_actor.to_account_info().key,
                &receiver.to_account_info().key,
                amount,
            ),
            &[
                bad_actor.to_account_info().clone(),
                receiver.to_account_info().clone(),
                system_program.to_account_info().clone(),
            ],
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(random: u32, loop_counter: u16, amount: u64)]
pub struct Spam<'info> {
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub bad_actor: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub receiver: AccountInfo<'info>,
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
