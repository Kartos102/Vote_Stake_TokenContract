use anchor_lang::prelude::*;
use anchor_spl::token::{self, TokenAccount};
use exo_token::cpi as exocpl;
use gcred_token::cpi as gcredcpl;
use exo_token::cpi::accounts::{ ProxyTransfer as ProxyTransferExo,ProxyMintTo as ProxyMintToExo };

use gcred_token::cpi::accounts::{ ProxyMintTo as ProxyMintToGcred };

use exo_token::program::ExoToken;
use gcred_token::program::GcredToken;

pub mod state;

const ACCOUNT_PREFIX: &[u8] = b"stake_account"; 


declare_id!("7Kw3cjT8KWXrUSvRDjUf316zDk9f6YXEArpyYqRHquQQ");

use state::*;


#[program]
pub mod staking_reward {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>,exo_address: Pubkey, gcred_address: Pubkey,bump: u8) -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;
        let user = &mut ctx.accounts.user;
        base_account.exo_address = exo_address;
        base_account.gcred_address = gcred_address;

        base_account.default_admin_role = *user.to_account_info().key;
        base_account.owner_role = *user.to_account_info().key;
        base_account.exo_role = exo_address;
        base_account.bump = bump;

        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: String, duration:u8) -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;
        let holder = &mut ctx.accounts.holder;
        let my_token_account = &mut ctx.accounts.my_token_account;
        let foundation_node_token_account = &mut ctx.accounts.foundation_node_token_account;

        let clock = &ctx.accounts.clock;

        let amount : u64= amount.parse().unwrap();

        if base_account.paused {
            return Err(ErrorCode::ErrorForPause.into());
        }

        if amount > my_token_account.amount {
            return Err(ErrorCode::NotEnoughExoToken.into());
        }

        if duration >= 4 {
            return Err(ErrorCode::DurationNotMatch.into());
        }

        let mut tier_holder: u8 = 0;

        for tier in base_account.tier.iter() {
            if *holder.to_account_info().key == tier.address {
                tier_holder = tier.value;
            }
        }
        
        let interest_rate: u8 = tier_holder * 4 + duration;

        if *holder.to_account_info().key == base_account.foundation_node {
            base_account.fn_reward = foundation_node_token_account.amount * 75 / 1000 /365;
        } else {
            let min_amount = get_tier_amount();
            let period = get_staking_period();

            let mut index = 1;
            for staking_info in base_account.staking_infos.iter() {
                if *holder.to_account_info().key  == staking_info.holder {
                    index = staking_info.index + 1;
                }
            }

            let new_staking_info:StakingInfo = StakingInfo {
                holder: *holder.to_account_info().key,
                amount,
                start_date: clock.unix_timestamp,
                expire_date: clock.unix_timestamp + period[duration as usize],
                duration: duration as i64 * 3600 * 24,
                claim_day:0,
                interest_rate,
                index
            };

            base_account.staking_infos.push(new_staking_info);
            base_account.total_staking += 1;

            base_account.interest_holder_counter[interest_rate as usize] += 1;

            if tier_holder < 3 && amount > min_amount[tier_holder as usize + 1] && duration > tier_holder {

                let tier_candidate_flag:bool = false;
                for tier_candidate in base_account.tier_candidate.iter_mut() {
                    if *holder.to_account_info().key == tier_candidate.address {
                        tier_candidate.value = true;
                    }
                }
                if !tier_candidate_flag {
                    base_account.tier_candidate.push(TierCandiate{
                        address: *holder.to_account_info().key,
                        value: false
                    });
                }
            }
        }

        let cpi_program = ctx.accounts.exo_token_program.to_account_info();

        let cpi_accounts = ProxyTransferExo {
            authority: holder.to_account_info(),
            from: holder.to_account_info(),
            to: ctx.accounts.base_account.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            base_account: ctx.accounts.exo_token_program_base_account.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        exocpl::proxy_transfer(cpi_ctx,amount.to_string()).expect("transfer has just been failed");

        emit!(StakeExo{
            holder: *holder.to_account_info().key,
            amount,
            interest_rate
        });

        Ok(())
    }

    pub fn unstake(ctx: Context<UnStake>, staking_index: u16) -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;
        let holder = &mut ctx.accounts.holder;
        let clock =  &ctx.accounts.clock;

        if base_account.paused {
            return Err(ErrorCode::ErrorForPause.into());
        }

        let mut target_staking  = base_account.staking_infos[0];
        let mut tier = base_account.tier[0];
        let mut tier_candidate = base_account.tier_candidate[0];

        let mut max_length = 0;

        {
            for staking_info in base_account.staking_infos.iter() {
                if *holder.to_account_info().key  == staking_info.holder {
                    max_length = staking_info.index;
                    if staking_info.index == staking_index {
                        target_staking = *staking_info;
                }
            }
        }

        if max_length <= staking_index {
            return Err(ErrorCode::InvalidStakingIndex.into());
        }

        if clock.unix_timestamp < target_staking.expire_date && target_staking.interest_rate % 4 != 0 {
            return Err(ErrorCode::UnStakingFailed.into());
        }

        //call the claim function
        let mut cur_date: i64 = 0;

        if clock.unix_timestamp >= target_staking.expire_date {
            cur_date = target_staking.expire_date;
        } else {
            cur_date = clock.unix_timestamp;
        }

        let staked_days: i64 = cur_date - target_staking.start_date;

        if staked_days > target_staking.claim_day {
            let reward_days : i64 = staked_days - target_staking.claim_day;
            let reward_apr = get_exo_reward_apr(target_staking.interest_rate);
            let reward = cal_reward(target_staking.amount, reward_apr as u64, reward_days);
            base_account.total_reward_amount += reward;

            let cpi_program = ctx.accounts.exo_token_program.to_account_info();

            let cpi_accounts = ProxyMintToExo {
                authority: ctx.accounts.authority.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: holder.to_account_info(),
                base_account: ctx.accounts.exo_token_program_base_account.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

    
            exocpl::proxy_mint(cpi_ctx,reward.to_string()).expect("Mint has just been failed");

            //send gcred

            let gcred_reward = (target_staking.amount * get_gcred_return(target_staking.interest_rate) as u64 * reward_days as u64) / 1000000;

            let cpi_program = ctx.accounts.gcred_token_program.to_account_info();

            let cpi_accounts = ProxyMintToGcred {
                authority: ctx.accounts.authority.to_account_info(),
                mint: ctx.accounts.gcred_mint.to_account_info(),
                to: holder.to_account_info(),
                base_account: ctx.accounts.exo_token_program_base_account.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);


            gcredcpl::proxy_mint_for_reward(cpi_ctx,gcred_reward.to_string()).expect("Mint has just been failed");

            emit!(ClaimGCRED{
                    holder: *holder.to_account_info().key, 
                    amount: gcred_reward
                }
            );

            target_staking.claim_day += reward_days;
            emit!(ClaimExo{
                holder: *holder.to_account_info().key,
                reward,
                interest_rate: target_staking.interest_rate
            });

            //getRewardFromFN
            let fn_reward_percent : Vec<u8> = get_fn_reward_percent();
            let mut reward_amount_fn = 0;
            //calculate daily fn reward
            if base_account.interest_holder_counter[target_staking.interest_rate as usize] == 0 {
                reward_amount_fn = 0;
            } else {
                reward_amount_fn = base_account.fn_reward * fn_reward_percent[target_staking.interest_rate as usize] as u64/ base_account.interest_holder_counter[target_staking.interest_rate as usize] as u64 / 1000;
            }

            let reward_amount = reward_amount_fn * reward_days as u64;
            base_account.total_reward_amount += reward_amount;

            if reward_amount > 0 {
                let cpi_program = ctx.accounts.exo_token_program.to_account_info();

                let cpi_accounts = ProxyMintToExo {
                    authority: ctx.accounts.authority.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: holder.to_account_info(),
                    base_account: ctx.accounts.exo_token_program_base_account.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                };
                let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        
                exocpl::proxy_mint(cpi_ctx,reward_amount.to_string()).expect("Mint has just been failed");

                emit!(ClaimFN{
                    holder: *holder.to_account_info().key,
                    reward_amount
                })
            }
        }
        //

        for temp_tier_candidate in base_account.tier_candidate.iter() {
            if *holder.to_account_info().key == tier_candidate.address {
                tier_candidate = *temp_tier_candidate;
            }
        }

        for temp_tier in base_account.tier.iter() {
            if *holder.to_account_info().key == temp_tier.address {
                tier = *temp_tier;
            }
        }

        if target_staking.duration >= tier.value as i64 * 3600 * 24 && tier_candidate.value {
            if tier.value < 3{
                tier.value += 1;
            }
            tier_candidate.value = false;
        }

        base_account.interest_holder_counter[target_staking.interest_rate as usize] -= 1;
        target_staking = base_account.staking_infos[max_length as usize-1];
        base_account.staking_infos.pop();
        base_account.total_staking -= 1;

        let cpi_program = ctx.accounts.exo_token_program.to_account_info();

        let base_account_seeds = &[
            &id().to_bytes(),
            ACCOUNT_PREFIX,
            &[base_account.bump],
        ];

        let base_account_signer = &[&base_account_seeds[..]];

        let cpi_accounts = ProxyTransferExo {
            authority: base_account.to_account_info(),
            from: base_account.to_account_info(),
            to: holder.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            base_account: ctx.accounts.exo_token_program_base_account.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts,base_account_signer);

        exocpl::proxy_transfer(cpi_ctx,target_staking.amount.to_string()).expect("transfer has just been failed");

        emit!(UnStakeExo{
            holder: *holder.to_account_info().key,
            amount: target_staking.amount,
            interest_rate: target_staking.interest_rate
        });
    }
        Ok(())
    }

    pub fn claim(ctx: Context<Claim>, staking_index: u16) -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;
        let holder = &mut ctx.accounts.holder;
        let clock = &ctx.accounts.clock;
        
        let mut target_staking: StakingInfo = StakingInfo::default();

        if base_account.paused {
            return Err(ErrorCode::ErrorForPause.into());
        }

        let mut max_length = 0;

        for staking_info in base_account.staking_infos.iter_mut() {
            if *holder.to_account_info().key  == staking_info.holder {
                max_length = staking_info.index;
                if staking_info.index == staking_index {
                    target_staking = *staking_info;

                }
            }
        }

        if max_length <= staking_index {
            return Err(ErrorCode::InvalidStakingIndex.into());
        }

        let mut cur_date: i64 = 0;

        if clock.unix_timestamp >= target_staking.expire_date {
            cur_date = target_staking.expire_date;
        } else {
            cur_date = clock.unix_timestamp;
        }

        let staked_days: i64 = cur_date - target_staking.start_date;

        if staked_days > target_staking.claim_day {
            let reward_days : i64 = staked_days - target_staking.claim_day;
            let reward_apr = get_exo_reward_apr(target_staking.interest_rate);
            let reward = cal_reward(target_staking.amount, reward_apr as u64, reward_days);
            base_account.total_reward_amount += reward;

            let cpi_program = ctx.accounts.exo_token_program.to_account_info();

            let cpi_accounts = ProxyMintToExo {
                authority: ctx.accounts.authority.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: holder.to_account_info(),
                base_account: ctx.accounts.exo_token_program_base_account.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

    
            exocpl::proxy_mint(cpi_ctx,reward.to_string()).expect("Mint has just been failed");

            //send gcred

            let gcred_reward = (target_staking.amount * get_gcred_return(target_staking.interest_rate) as u64 * reward_days as u64) / 1000000;

            let cpi_program = ctx.accounts.gcred_token_program.to_account_info();

            let cpi_accounts = ProxyMintToGcred {
                authority: ctx.accounts.authority.to_account_info(),
                mint: ctx.accounts.gcred_mint.to_account_info(),
                to: holder.to_account_info(),
                base_account: ctx.accounts.exo_token_program_base_account.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);


            gcredcpl::proxy_mint_for_reward(cpi_ctx,gcred_reward.to_string()).expect("Mint has just been failed");

            emit!(ClaimGCRED{
                    holder:*holder.to_account_info().key, 
                    amount:gcred_reward
                }
            );

            target_staking.claim_day += reward_days;
            emit!(ClaimExo{
                holder: *holder.to_account_info().key,
                reward,
                interest_rate : target_staking.interest_rate
            });

            //getRewardFromFN
            let fn_reward_percent : Vec<u8> = get_fn_reward_percent();
            let mut reward_amount_fn = 0;
            //calculate daily fn reward
            if base_account.interest_holder_counter[target_staking.interest_rate as usize] == 0 {
                reward_amount_fn = 0;
            } else {
                reward_amount_fn = base_account.fn_reward * fn_reward_percent[target_staking.interest_rate as usize] as u64/ base_account.interest_holder_counter[target_staking.interest_rate as usize] as u64 / 1000;
            }

            let reward_amount = reward_amount_fn * reward_days as u64;
            base_account.total_reward_amount += reward_amount;

            if reward_amount > 0 {
                let cpi_program = ctx.accounts.exo_token_program.to_account_info();

                let cpi_accounts = ProxyMintToExo {
                    authority: ctx.accounts.authority.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: holder.to_account_info(),
                    base_account: ctx.accounts.exo_token_program_base_account.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                };
                let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        
                exocpl::proxy_mint(cpi_ctx,reward_amount.to_string()).expect("Mint has just been failed");

                emit!(ClaimFN{
                    holder: *holder.to_account_info().key,
                    reward_amount
                });
            }
        }

        Ok(())
    }

    pub fn pause(ctx: Context<UpdatePauseRoleOrAddressOrInfo>) -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;
        let user = &ctx.accounts.user;

        if base_account.owner_role != *user.to_account_info().key {
            return Err(ErrorCode::NotOwnerAccount.into());
        }

        base_account.paused = true;
        
        Ok(())
    }
    
    pub fn unpause(ctx: Context<UpdatePauseRoleOrAddressOrInfo>) -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;
        let user = &ctx.accounts.user;

        if base_account.owner_role != *user.to_account_info().key {
            return Err(ErrorCode::NotOwnerAccount.into());
        }

        base_account.paused = false;
        
        Ok(())
    }

    pub fn set_gcred_address(ctx: Context<UpdatePauseRoleOrAddressOrInfo>, gcred_address: Pubkey) -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;
        let user = &ctx.accounts.user;

        if base_account.owner_role != *user.to_account_info().key {
            return Err(ErrorCode::NotOwnerAccount.into());
        }

        base_account.gcred_address = gcred_address;

        emit!(GCREDAddressUpdated {
            gcred_address
        });
        
        Ok(())
    }

    pub fn set_fn_address(ctx: Context<UpdatePauseRoleOrAddressOrInfo>, foundation_node: Pubkey) -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;
        let user = &ctx.accounts.user;

        if base_account.owner_role != *user.to_account_info().key {
            return Err(ErrorCode::NotOwnerAccount.into());
        }

        base_account.foundation_node = foundation_node;

        emit!(FoundationNodeUpdated {
            foundation_node
        });
        
        Ok(())
    }

    pub fn set_exo_address(ctx: Context<UpdatePauseRoleOrAddressOrInfo>, exo_address: Pubkey) -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;
        let user = &ctx.accounts.user;

        if base_account.owner_role != *user.to_account_info().key {
            return Err(ErrorCode::NotOwnerAccount.into());
        }

        base_account.exo_address = exo_address;

        emit!(ExoAddressUpdated {
            exo_address
        });
        
        Ok(())
    }

    pub fn set_tier(ctx: Context<UpdatePauseRoleOrAddressOrInfo>,holder: Pubkey,tier: u8)  -> Result<()> {
        let base_account = &mut ctx.accounts.base_account;

        if base_account.paused {
            return Err(ErrorCode::ErrorForPause.into());
        }

        if base_account.owner_role != holder {
            return Err(ErrorCode::NotOwnerAccount.into());
        }

        let mut flag = false;

        for tier_holder in base_account.tier.iter_mut() {
            if tier_holder.address == holder {
                tier_holder.value = tier;
                flag = true;
            }
        }

        if !flag {
            base_account.tier.push(Tier{
                address: holder,
                value: tier,
            });
        }

        Ok(())
    }

    pub fn get_tier(ctx: Context<UpdatePauseRoleOrAddressOrInfo>,holder: Pubkey) -> Result<u8> {
        let base_account = &ctx.accounts.base_account;

        let mut value : u8 = 0;

        for tier_holder in base_account.tier.iter() {
            if tier_holder.address == holder {
                value = tier_holder.value;
            }
        }

        Ok(value)
    }

    pub fn get_staking_info(ctx: Context<UpdatePauseRoleOrAddressOrInfo>,holder: Pubkey) -> Result<Vec<StakingInfo>> {
        let base_account = &mut ctx.accounts.base_account;

        let mut  infos: Vec<StakingInfo> = Vec::new();

        for staking_info in base_account.staking_infos.iter() {
            if staking_info.holder == holder {
                infos.push(*staking_info);
            }
        }

        Ok(infos)
    }

    pub fn get_total_staing_amount(ctx: Context<UpdatePauseRoleOrAddressOrInfo>,holder: Pubkey) -> Result<u64> {
        let base_account = &mut ctx.accounts.base_account;

        let mut amount: u64 = 0;

        for staking_info in base_account.staking_infos.iter() {
            if staking_info.holder == holder {
                amount += staking_info.amount;
            }
        }

        Ok(amount)
    }    
}

pub fn get_tier_amount() -> Vec<u64> {
    return vec![0,
        2000000000000000,
        4000000000000000,
        8000000000000000
    ];
}

fn get_staking_period() -> Vec<i64> {
    return vec![
        0,
        30 * 3600 * 24,
        60 * 3600 * 24,
        90 * 3600 * 24
    ];
}

fn get_exo_reward_apr(interest_rate: u8) -> u8 {
    let exo_reward_apr = vec![50,55,60,65,60,65,70,75,60,65,70,75,60,65,70,75];
    return exo_reward_apr[interest_rate as usize];
}

fn get_fn_reward_percent() -> Vec<u8> {
    let fn_reward_percent  =vec![0, 0, 0, 0, 30, 60, 85, 115, 40, 70, 95, 125, 50, 80, 105, 145];
    return fn_reward_percent;
}

fn get_gcred_return(interest: u8) -> u16 {
    let gcred_return = vec![0, 0, 0, 242, 0, 0, 266, 354, 0, 0, 293, 390, 0, 0, 322, 426];
    return gcred_return[interest as usize];
}

fn cal_reward(amount: u64, percent :u64, reward_days: i64) -> u64 {
    return amount * percent * reward_days as u64 / 365000 / (3600 * 24);
}

#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = user,
        space = 10000,
        seeds = [&id().to_bytes(), &user.key().to_bytes(),ACCOUNT_PREFIX],
        bump
    )]
    pub base_account: Account<'info,BaseAccount>,
    #[account(mut)]
    pub user:Signer<'info>,
    pub system_program: Program <'info,System>,
    pub rent: Sysvar<'info, Rent>,
}


#[derive(Accounts)]
pub struct Stake<'info> {
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(signer)]
    pub holder: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [&id().to_bytes(), &holder.key().to_bytes(),ACCOUNT_PREFIX],
        bump = base_account.bump,
    )]
    pub base_account: Account<'info, BaseAccount>,

    pub exo_token_program: Program<'info, ExoToken>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub exo_token_program_base_account: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub my_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub foundation_node_token_account: Account<'info, TokenAccount>,
    pub mint: Account<'info, TokenAccount>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub token_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub clock: Sysvar<'info, Clock>,
}


#[derive(Accounts)]
pub struct UnStake<'info> {
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(signer)]
    pub holder: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [&id().to_bytes(), &holder.key().to_bytes(),ACCOUNT_PREFIX],
        bump = base_account.bump,
    )]
    pub base_account: Account<'info, BaseAccount>,

    #[account(mut)] 
    pub stake_token_account: Account<'info, TokenAccount>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(
        mut
    )]
    pub authority: AccountInfo<'info>,

    pub exo_token_program: Program<'info, ExoToken>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub exo_token_program_base_account: AccountInfo<'info>,
    pub gcred_token_program: Program<'info, GcredToken>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub gcred_token_program_base_account: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub mint: Account<'info, TokenAccount>,
    pub gcred_mint: Account<'info,TokenAccount>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub token_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(signer)]
    pub holder: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [&id().to_bytes(), &holder.key().to_bytes(),ACCOUNT_PREFIX],
        bump = base_account.bump,
    )]
    pub base_account: Account<'info, BaseAccount>,

    #[account(mut)] 
    pub stake_token_account: Account<'info, TokenAccount>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(mut)]
    pub authority: AccountInfo<'info>,

    pub exo_token_program: Program<'info, ExoToken>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub exo_token_program_base_account: AccountInfo<'info>,
    pub gcred_token_program: Program<'info, GcredToken>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub gcred_token_program_base_account: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub mint: Account<'info, TokenAccount>,
    pub gcred_mint: Account<'info, TokenAccount>,

    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub token_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UpdatePauseRoleOrAddressOrInfo<'info> {
    #[account(
        mut,
        seeds = [&id().to_bytes(), ACCOUNT_PREFIX],
        bump = base_account.bump,
    )]
    pub base_account: Account<'info, BaseAccount>,
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(signer)]
    pub user: AccountInfo<'info>,
}

#[event]
pub struct StakeExo {
    pub holder: Pubkey,
    pub amount: u64,
    pub interest_rate: u8,
}

#[event]
pub struct UnStakeExo {
    pub holder: Pubkey,
    pub amount: u64,
    pub interest_rate: u8,
}

#[event]
pub struct GCREDAddressUpdated {
    pub gcred_address: Pubkey,
}

#[event]
pub struct FoundationNodeUpdated {
    pub foundation_node: Pubkey,
}

#[event]
pub struct ClaimGCRED {
    pub holder: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ClaimExo {
    pub holder: Pubkey,
    pub reward: u64,
    pub interest_rate: u8,
}

#[event]
pub struct ClaimFN {
    pub holder: Pubkey,
    pub reward_amount: u64,
}

#[event]
pub struct ExoAddressUpdated {
    pub exo_address: Pubkey,
}

#[error_code]
pub enum ErrorCode {
    #[msg("You must be owner!")]
    NotOwnerAccount,
    #[msg("Not enough EXO token to stake!")]
    NotEnoughExoToken,
    #[msg("Duration does not match")]
    DurationNotMatch,
    #[msg("The contract is paused!")]
    ErrorForPause,
    #[msg("Invalid staking index!")]
    InvalidStakingIndex,
    #[msg("Cannot unstake!")]
    UnStakingFailed
}

