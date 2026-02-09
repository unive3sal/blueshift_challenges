import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { BlueshiftAnchorFlashLoan } from "../target/types/blueshift_anchor_flash_loan";

describe("blueshift_anchor_flash_loan", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.blueshiftAnchorFlashLoan as Program<BlueshiftAnchorFlashLoan>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
