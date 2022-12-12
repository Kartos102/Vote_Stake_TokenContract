const anchor = require("@project-serum/anchor");
const assert = require("assert");
const { SystemProgram } = anchor.web3;

describe("GCREDToken", () => {
  // Configure the client to use the local cluster.
  // anchor.setProvider(anchor.AnchorProvider.env());
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.GcredToken;
  const md_account = anchor.web3.Keypair.generate();
  const dao_account = anchor.web3.Keypair.generate();
  
  let mint = null;
  let from = null;
  let to = null;
  const baseAccount = anchor.web3.Keypair.generate();

  it("Is initialized!", async () => {
    // Add your test here.
 
    const tx = await program.rpc.initialize(md_account.publicKey,dao_account.publicKey,{
      accounts:{
        baseAccount: baseAccount.publicKey,
        user: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      },
      signers: [baseAccount]
    });
    let baseAccount1 = await program.account.baseAccount.fetch(baseAccount.publicKey);
    console.log("Your transaction signature", tx);
    assert.ok(md_account.publicKey.equals(baseAccount1.mdAccount))
    assert.ok(dao_account.publicKey.equals(baseAccount1.daoAccount))
  });
  it("Initializes test state", async () => {
    mint = await createMint(provider);
    from = await createTokenAccount(provider, mint, provider.wallet.publicKey);
    to = await createTokenAccount(provider, mint, provider.wallet.publicKey);
  });

  it("unpause the contract",async () => {
    await program.rpc.unpause({
      accounts:{
        baseAccount:baseAccount.publicKey,
        user: provider.wallet.publicKey,
      },
    });
  });

  it("Mints a token", async () => {
    await program.rpc.proxyMint('1000000000000', {
      accounts: {
        authority: provider.wallet.publicKey,
        mint,
        to: from,
        baseAccount:baseAccount.publicKey,
        tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID,
      },
    });

    const fromAccount = await getTokenAccount(provider, from);

    assert.ok(fromAccount.amount.eq(new anchor.BN(1000000000000)));
  });
  it("set the brige role",async () => {
    await program.rpc.updateRole(provider.wallet.publicKey,1,{
      accounts:{
        baseAccount:baseAccount.publicKey,
        user: provider.wallet.publicKey,
      },
    });
  });
  it("set the staking reward role",async () => {
    await program.rpc.updateRole(provider.wallet.publicKey,2,{
      accounts:{
        baseAccount:baseAccount.publicKey,
        user: provider.wallet.publicKey,
      },
    });
  });

  it("Burns a token", async () => {
    await program.rpc.proxyBridgeBurn('350000000000', {
      accounts: {
        authority: provider.wallet.publicKey,
        mint,
        to:from,
        baseAccount:baseAccount.publicKey,
        tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID,
      },
    });

    const fromAccount = await getTokenAccount(provider, from);
    assert.ok(fromAccount.amount.eq(new anchor.BN(650000000000)));
  });

  it("Transfers a token", async () => {
  const md = await createTokenAccount(provider, mint, md_account.publicKey);


    await program.rpc.proxyTransfer('400000000000', {
      accounts: {
        authority: provider.wallet.publicKey,
        from,
        to,
        md,
        mint,
        baseAccount:baseAccount.publicKey,
        tokenProgram: TokenInstructions.TOKEN_PROGRAM_ID,
      },
    });
  
    const fromAccount = await getTokenAccount(provider, from);
    const toAccount = await getTokenAccount(provider, md);
    assert.ok(fromAccount.amount.eq(new anchor.BN(250000000000)));
  });
});


const serumCmn = require("@project-serum/common");
const TokenInstructions = require("@project-serum/serum").TokenInstructions;

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
