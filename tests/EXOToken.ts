import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { ExoToken } from "../target/types/exo_token";

describe("EXOToken", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.ExoToken as Program<ExoToken>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
