const anchor = require("@project-serum/anchor");
const bs58 = require("bs58");
const { Connection, LAMPORTS_PER_SOL } = require("@solana/web3.js");

describe("StakingReward", async () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const gcred_program = anchor.workspace.GcredToken;
  const brige_program = anchor.workspace.Bridge;
  const exo_program = anchor.workspace.ExoToken;
  const staking_program = anchor.workspace.StakingReward;
  
  const md_account = anchor.web3.Keypair.generate();
  const dao_account = anchor.web3.Keypair.generate();

  const wallet_Account = anchor.web3.Keypair.generate();

  const systemProgram = anchor.web3.SystemProgram.programId;
  const rentSysvar = anchor.web3.SYSVAR_RENT_PUBKEY;
  const clockSysvar = anchor.web3.SYSVAR_CLOCK_PUBKEY;
  
  let gcred_mint = null;
  let exo_mint = null;
  let from = null;
  let to = null;
  let listener = null;

  let baseAccountStaking = null;
  let baseAccountStakingBump = null;

  const baseAccountGcred = anchor.web3.Keypair.generate();
  const baseAccountBridge = anchor.web3.Keypair.generate();
  const baseAccountExo = anchor.web3.Keypair.generate();

  const owner = anchor.web3.Keypair.generate();
  
  // before(() => {
  //   owner = anchor.web3.Keypair.fromSecretKey(
  //     bs58.decode(
  //       "5xHmwFrmzD5m3cgkCKmrhxks8XJjvP4PLm38cAYHhJYPifgE1ivKEbsZAb7uv6DNzhRBE44prMWghj3g5Kr7WTwx"
  //     )
  //   );
  //   }
  // );

  it("Initializes test state", async () => {
    gcred_mint = await createMint(provider);
    console.log("GCRED Token Address : ",gcred_mint.toString());
    exo_mint = await createMint(provider);
    console.log("EXO Token Address : ",exo_mint.toString());
    // from = await createTokenAccount(provider, exo_mint, provider.wallet.publicKey);
    // to = await createTokenAccount(provider, exo_mint, provider.wallet.publicKey);

  });

  it("Is initialized!", async () => {
    // Add your test here.
    [baseAccountStaking, baseAccountStakingBump] =
    await anchor.web3.PublicKey.findProgramAddress(
      [
        staking_program.programId.toBuffer(),
        provider.wallet.publicKey.toBuffer(),
        Buffer.from("stake_account"),
      ],
      staking_program.programId
    );

    
    await staking_program.rpc.initialize(exo_mint,gcred_mint,baseAccountStakingBump,{
        accounts:{
          baseAccount: baseAccountStaking,
          user: provider.wallet.publicKey,
          systemProgram,
          rent: rentSysvar,
        },
        signers:[provider.wallet.payer],
    });

    // await gcred_program.rpc.initialize(md_account.publicKey,dao_account.publicKey,{
    //     accounts:{
    //         baseAccount: baseAccountGcred.publicKey,
    //         user: owner.publicKey,
    //         systemProgram: SystemProgram.programId,
    //     },
    //     signers:[owner],
    // });

    // await exo_program.rpc.initialize({
    //     accounts:{
    //         baseAccount: baseAccountExo.publicKey,
    //         user: owner.publicKey,
    //         systemProgram: SystemProgram.programId,
    //     },
    //     signers:[owner],
    // });

  });

  // it("unpause the staking contract",async () => {
  //   await staking_program.rpc.unpause({
  //     accounts:{
  //       baseAccount:new anchor.web3.PublicKey(baseAccountStaking),
  //       user: owner.publicKey,
  //     },
  //   });
  //   console.log("unpausing gcred program has just been completed")
  // });

});

const serumCmn = require("@project-serum/common");
const { ACCOUNT_DISCRIMINATOR_SIZE } = require("@project-serum/anchor");
const TokenInstructions = require("@project-serum/serum").TokenInstructions;

const sleep = (ms) => {
  return new Promise((resolve)=> setTimeout(resolve,ms));
}

const TOKEN_PROGRAM_ID = new anchor.web3.PublicKey(
  TokenInstructions.TOKEN_PROGRAM_ID.toString()
);

async function getTokenAccount(provider, addr) {
  return await serumCmn.getTokenAccount(provider, addr);
}

async function getMintInfo(provider, mintAddr) {
  return await serumCmn.getMintInfo(provider, mintAddr);
}

async function createMint(provider, authority) {
  if (authority === undefined) {
    authority = provider.wallet.publicKey;
  }
  const mint = anchor.web3.Keypair.generate();
  const instructions = await createMintInstructions(
    provider,
    authority,
    mint.publicKey
  );

  const tx = new anchor.web3.Transaction();
  tx.add(...instructions);


  await anchor.AnchorProvider.env().sendAndConfirm(tx, [mint]);

  return mint.publicKey;
}

async function createMintInstructions(provider, authority, mint) {
  let instructions = [
    anchor.web3.SystemProgram.createAccount({
      fromPubkey: provider.wallet.publicKey,
      newAccountPubkey: mint,
      space: 82,
      lamports: await provider.connection.getMinimumBalanceForRentExemption(82),
      programId: TOKEN_PROGRAM_ID,
    }),
    TokenInstructions.initializeMint({
      mint,
      decimals: 0,
      mintAuthority: authority,
    }),
  ];
  return instructions;
}

async function createTokenAccount(provider, mint, owner) {
  const vault = anchor.web3.Keypair.generate();
  const tx = new anchor.web3.Transaction();
  tx.add(
    ...(await createTokenAccountInstrs(provider, vault.publicKey, mint, owner))
  );
  await anchor.AnchorProvider.env().sendAndConfirm(tx, [vault]);
  return vault.publicKey;
}

async function createTokenAccountInstrs(
  provider,
  newAccountPubkey,
  mint,
  owner,
  lamports
) {
  if (lamports === undefined) {
    lamports = await provider.connection.getMinimumBalanceForRentExemption(165);
  }
  return [
    anchor.web3.SystemProgram.createAccount({
      fromPubkey: provider.wallet.publicKey,
      newAccountPubkey,
      space: 165,
      lamports,
      programId: TOKEN_PROGRAM_ID,
    }),
    TokenInstructions.initializeAccount({
      account: newAccountPubkey,
      mint,
      owner,
    }),
  ];
}
