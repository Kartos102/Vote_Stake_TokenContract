const anchor = require("@project-serum/anchor");
const assert = require("assert");
const { SystemProgram } = anchor.web3;
const {Connection,clusterApiUrl} = require('@solana/web3.js');
const opts = {
  preflightCommitment: "processed"
}
const network = clusterApiUrl('devnet');

describe("Bridge", () => {
  // Configure the client to use the local cluster.
  // anchor.setProvider(anchor.AnchorProvider.env());
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const gcred_program = anchor.workspace.GcredToken;
  const brige_program = anchor.workspace.Bridge;
  const exo_program = anchor.workspace.ExoToken;
  
  const md_account = anchor.web3.Keypair.generate();
  const dao_account = anchor.web3.Keypair.generate();

  const wallet_Account = anchor.web3.Keypair.generate();
  
  let mint = null;
  let exo_mint = null;
  let from = null;
  let to = null;
  let listener = null;
  const connection = new Connection(network, opts.preflightCommitment);

  const baseAccount = anchor.web3.Keypair.generate();
  const baseAccountBridge = anchor.web3.Keypair.generate();
  const baseAccountExo = anchor.web3.Keypair.generate();

  it("Initializes test state", async () => {
    mint = await createMint(provider);
    console.log("GCRED Token Address : ",mint.toString());
    exo_mint = await createMint(provider);
    console.log("EXO Token Address : ",exo_mint.toString());
    from = await createTokenAccount(provider, mint, provider.wallet.publicKey);
    to = await createTokenAccount(provider, mint, provider.wallet.publicKey);
  });

  it("Is initialized!", async () => {
    // Add your test here.
 
    const tx = await gcred_program.rpc.initialize(md_account.publicKey,dao_account.publicKey,{
      accounts:{
        baseAccount: baseAccount.publicKey,
        user: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      },
      signers: [baseAccount]
    });

    const tx_1 = await brige_program.rpc.initialize(
      {
        accounts: {
          baseAccount: baseAccountBridge.publicKey,
          exoMint:exo_mint,
          gcredMint:mint,
          user:provider.wallet.publicKey,
          systemProgram: SystemProgram.programId
        },  
        signers:[baseAccountBridge]
      }
    );

    let baseAccount1 = await gcred_program.account.baseAccount.fetch(baseAccount.publicKey);
    console.log("GCRED program was initialized", tx);
    console.log("Bridge program was initialized", tx_1);

    assert.ok(md_account.publicKey.equals(baseAccount1.mdAccount))
    assert.ok(dao_account.publicKey.equals(baseAccount1.daoAccount))
  });
  

  it("unpause the contract",async () => {
    await gcred_program.rpc.unpause({
      accounts:{
        baseAccount:baseAccount.publicKey,
        user: provider.wallet.publicKey,
      },
    });
    console.log("unpausing gcred program has just been completed")
  });

  it("unpause the brige contract",async () => {
    await brige_program.rpc.unpause({
      accounts:{
        baseAccount:baseAccountBridge.publicKey,
        user: provider.wallet.publicKey,
      },
    });
    console.log("unpausing the bridge contract has just been completed")
  });

  it("Mints a token", async () => {
    await gcred_program.rpc.proxyMint('1000000000000', {
      accounts: {
        authority: provider.wallet.publicKey,
        mint,
        to: from,
        baseAccount:baseAccount.publicKey,
        tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID,
      },
    });

    const fromAccount = await getTokenAccount(provider, from);

    console.log("gcred token 1000 has just been minted to ", from.toString() +", not bridge");

    assert.ok(fromAccount.amount.eq(new anchor.BN(1000000000000)));
  });

  it("set the brige role",async () => {
    await gcred_program.rpc.updateRole(provider.wallet.publicKey,1,{
      accounts:{
        baseAccount:baseAccount.publicKey,
        user: provider.wallet.publicKey,
      },
    });
    console.log("this account " + provider.wallet.publicKey.toString()+" has the authority to call the bridge function");
  });

  it("set the staking reward role",async () => {
    await gcred_program.rpc.updateRole(provider.wallet.publicKey,2,{
      accounts:{
        baseAccount:baseAccount.publicKey,
        user: provider.wallet.publicKey,
      },
    });
    console.log("this account " + provider.wallet.publicKey.toString()+" has the authority to call the staking function");
  });

  it("Bridge Mint",async () => {
    
    
    let [event, slot] = await new Promise((resolve, _reject) => {
      listener = brige_program.addEventListener("Transfer", (event, slot) => {
        resolve([event, slot]);
      });
      brige_program.rpc.proxyBridgeMint('100000000000','0x123123123',{
        accounts:{
          authority: provider.wallet.publicKey,
          mint,
          to: to,
          baseAccount: baseAccountBridge.publicKey,
          tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID,
          exoTokenProgram: exo_program.programId,
          exoTokenProgramBaseAccount:baseAccountExo.publicKey,
          gcredTokenProgram: gcred_program.programId,
          gcredTokenProgramBaseAccount: baseAccount.publicKey
        }
      })
    });
    console.log("emit logo")
    console.log(event,slot);
    await brige_program.removeEventListener(listener);
    const toAccount = await getTokenAccount(provider, to);

    console.log("gcred token "+toAccount.amount/1000000000 + " has just been minted to ", to.toString()+",bridge mint");

    assert.ok(toAccount.amount.eq(new anchor.BN(100000000000)));
  })

  it("Burns a token via bridge", async () => {
    let [event, slot] = await new Promise((resolve, _reject) => {
      listener = brige_program.addEventListener("Transfer", (event, slot) => {
        resolve([event, slot]);
      });
      brige_program.rpc.proxyBridgeBurn('50000000000','0x123123123', {
        accounts: {
          authority: provider.wallet.publicKey,
          mint,
          to: to,
          baseAccount: baseAccountBridge.publicKey,
          tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID,
          exoTokenProgram: exo_program.programId,
          exoTokenProgramBaseAccount:baseAccountExo.publicKey,
          gcredTokenProgram: gcred_program.programId,
          gcredTokenProgramBaseAccount: baseAccount.publicKey
        },
      });
    });
    console.log("emit logo")
    console.log(event,slot);
    await brige_program.removeEventListener(listener);

    console.log("gcred token 50 has just been burned in ", to.toString() +", bridge burn");

    const toAccount = await getTokenAccount(provider, to);
    console.log(to.toString() + "'s balance is  " + toAccount.amount/1000000000);

    assert.ok(toAccount.amount.eq(new anchor.BN(50000000000)));
  });

  // it("Transfers a token", async () => {
  // const md = await createTokenAccount(provider, mint, md_account.publicKey);


  //   await gcred_program.rpc.proxyTransfer('400000000000', {
  //     accounts: {
  //       authority: provider.wallet.publicKey,
  //       from,
  //       to,
  //       md,
  //       mint,
  //       baseAccount:baseAccount.publicKey,
  //       tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID,
  //     },
  //   });
  
  //   const fromAccount = await getTokenAccount(provider, from);
  //   const toAccount = await getTokenAccount(provider, md);
  //   assert.ok(fromAccount.amount.eq(new anchor.BN(250000000000)));
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
