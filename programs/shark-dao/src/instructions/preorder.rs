use std::mem::size_of;
use anchor_lang::context::Context;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_program;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::Token;
use anchor_spl::token_2022::TransferChecked;
use anchor_spl::token_interface;
use anchor_spl::token_interface::{Mint, TokenAccount};
use crate::errs::ErrorCode;
use crate::instructions::{events, STATE_SEED, PREORDER, PreOrder, State};

pub const USER_PREORDER: &str = "USER_PREORDER";

/*
一共分3轮预售，第一轮价格0.005，预售0.6亿枚；第二轮价格0.008，数量0.9亿枚，第三轮价格0.1，数量1亿枚，预售计划一共买出2.5亿枚
*/
// 预购代币
pub(crate) fn preorder_token(ctx: Context<PreorderToken>, preorder_name: String, amount: u64) -> Result<()> {
    require!(ctx.accounts.state.init, ErrorCode::NotInit);
    let now = ctx.accounts.clock.unix_timestamp as u64;
    require!(ctx.accounts.preorder.stm < now && ctx.accounts.preorder.etm > now, ErrorCode::TimeOver);

    require!(ctx.accounts.user_collection_token_account.amount >= amount, ErrorCode::InsufficientCollectionMintBalance);

    let decimals = 10u64.pow(ctx.accounts.mint.decimals as u32);
    let buy_amount = amount.checked_mul(decimals).unwrap().checked_div(ctx.accounts.preorder.price).unwrap();

    require!(buy_amount > 0, ErrorCode::InvalidParameter);
    require!(ctx.accounts.preorder_token_account.amount > buy_amount, ErrorCode::InsufficientMintBalance);

    // transfer collection
    token_interface::transfer_checked(
        CpiContext::new(ctx.accounts.token_program.to_account_info(),TransferChecked {
            from: ctx.accounts.user_collection_token_account.to_account_info(),
            to: ctx.accounts.state_collection_token_account.to_account_info(),
            authority: ctx.accounts.payer.to_account_info(),
            mint: ctx.accounts.collection_mint.to_account_info(),
        }), amount, ctx.accounts.collection_mint.decimals)?;

    // transfer token
    token_interface::transfer_checked(
        CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(),TransferChecked {
            from: ctx.accounts.preorder_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.preorder.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
        }, &[
            &[
                PREORDER.as_bytes(),
                preorder_name.as_bytes(),
                &[ctx.bumps.preorder]
            ]
        ]), buy_amount, ctx.accounts.mint.decimals)?;

    msg!("#preorder token preorder_name: {} account: {}, amount: {}, buy_amount: {}", preorder_name, ctx.accounts.payer.key(), amount, buy_amount);

    if ctx.accounts.user_preorder.owner.eq(&system_program::id()) {
        ctx.accounts.user_preorder.owner = ctx.accounts.payer.key();
        ctx.accounts.user_preorder.mint = ctx.accounts.mint.key();
        ctx.accounts.user_preorder.collection_mint = ctx.accounts.collection_mint.key();
        ctx.accounts.user_preorder.ctm = ctx.accounts.clock.unix_timestamp as u64;
        ctx.accounts.user_preorder.extend = [0u64; 16];
        ctx.accounts.user_preorder.amount = amount;
        ctx.accounts.user_preorder.buy_amount = buy_amount;
        ctx.accounts.preorder.num += 1;
    }
    ctx.accounts.preorder.amount -= buy_amount;
    ctx.accounts.preorder.collection_amount += amount;

    emit!(events::Preorder{
            account: ctx.accounts.payer.key().to_string(),
            in_amount: amount,
            out_amount: buy_amount,
        });
    Ok(())
}

#[derive(Accounts)]
#[instruction(preorder_name: String)]
pub struct PreorderToken<'info> {
    #[account(mut, seeds = [STATE_SEED.as_bytes()], bump)]
    pub state: Box<Account<'info, State>>,
    #[account(init_if_needed, seeds = [USER_PREORDER.as_bytes(), preorder.key().as_ref(), payer.key().as_ref()], bump, payer = payer, space = size_of::<UserPreOrder>()+8)]
    pub user_preorder: Box<Account<'info, UserPreOrder>>,
    #[account(mut,
    seeds = [
    PREORDER.as_bytes(),
    preorder_name.as_bytes()
    ],
    bump)
    ]
    pub preorder: Box<Account<'info, PreOrder>>,
    // 预售token
    #[account(address = preorder.mint)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    // 收款token
    #[account(address = preorder.collection_mint)]
    pub collection_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = preorder,
        associated_token::token_program = token_program
    )]
    pub preorder_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = collection_mint,
        associated_token::authority = state,
        associated_token::token_program = token_program
    )]
    pub state_collection_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = collection_mint,
        associated_token::authority = payer,
        associated_token::token_program = token_program
    )]
    pub user_collection_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = payer,
        associated_token::token_program = token_program
    )]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}


#[account]
pub struct UserPreOrder {
    // 花费数量
    pub amount: u64,
    // 购买数量
    pub buy_amount: u64,
    // 预售时间
    pub ctm: u64,
    pub mint: Pubkey,
    pub collection_mint: Pubkey,
    pub owner: Pubkey,
    // 扩展
    pub extend: [u64; 16],
}