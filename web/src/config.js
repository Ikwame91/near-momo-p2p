// Public deployment values (testnet only). Override with VITE_* variables, see .env.example.
export const ESCROW_ID = import.meta.env.VITE_ESCROW_ID || "escrow-d2.ikwame.testnet";
export const DEFAULT_LP = import.meta.env.VITE_DEFAULT_LP || "ikwame.testnet";
export const RPC_URL = import.meta.env.VITE_RPC_URL || "https://test.rpc.fastnear.com";
export const EXPLORER_TX = "https://testnet.nearblocks.io/txns/";
