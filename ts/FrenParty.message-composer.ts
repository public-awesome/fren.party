/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.35.3.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

import { MsgExecuteContractEncodeObject } from "@cosmjs/cosmwasm-stargate";
import { MsgExecuteContract } from "cosmjs-types/cosmwasm/wasm/v1/tx";
import { toUtf8 } from "@cosmjs/encoding";
import { Decimal, InstantiateMsg, ExecuteMsg, Uint128, QueryMsg, Coin, Addr, Config } from "./FrenParty.types";
export interface FrenPartyMsg {
  contractAddress: string;
  sender: string;
  buyShares: ({
    amount,
    subject
  }: {
    amount: Uint128;
    subject: string;
  }, _funds?: Coin[]) => MsgExecuteContractEncodeObject;
  sellShares: ({
    amount,
    subject
  }: {
    amount: Uint128;
    subject: string;
  }, _funds?: Coin[]) => MsgExecuteContractEncodeObject;
}
export class FrenPartyMsgComposer implements FrenPartyMsg {
  sender: string;
  contractAddress: string;

  constructor(sender: string, contractAddress: string) {
    this.sender = sender;
    this.contractAddress = contractAddress;
    this.buyShares = this.buyShares.bind(this);
    this.sellShares = this.sellShares.bind(this);
  }

  buyShares = ({
    amount,
    subject
  }: {
    amount: Uint128;
    subject: string;
  }, _funds?: Coin[]): MsgExecuteContractEncodeObject => {
    return {
      typeUrl: "/cosmwasm.wasm.v1.MsgExecuteContract",
      value: MsgExecuteContract.fromPartial({
        sender: this.sender,
        contract: this.contractAddress,
        msg: toUtf8(JSON.stringify({
          buy_shares: {
            amount,
            subject
          }
        })),
        funds: _funds
      })
    };
  };
  sellShares = ({
    amount,
    subject
  }: {
    amount: Uint128;
    subject: string;
  }, _funds?: Coin[]): MsgExecuteContractEncodeObject => {
    return {
      typeUrl: "/cosmwasm.wasm.v1.MsgExecuteContract",
      value: MsgExecuteContract.fromPartial({
        sender: this.sender,
        contract: this.contractAddress,
        msg: toUtf8(JSON.stringify({
          sell_shares: {
            amount,
            subject
          }
        })),
        funds: _funds
      })
    };
  };
}