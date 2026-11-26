import { describe, expect, it } from 'vitest'
import type { ChainAccount, ChainOverview } from '~/composables/useAdminApi'
import { claimsLeft, rentSurplus, settlesLeft, tokenUnits } from './adminChain'

function account(overrides: Partial<ChainAccount>): ChainAccount {
  return {
    key: 'faucet',
    address: 'Faucet1111111111111111111111111111111111111',
    exists: true,
    lamports: 0,
    data_len: 0,
    rent_exempt_lamports: 890_880,
    owner: '11111111111111111111111111111111',
    executable: false,
    tokens: null,
    ...overrides
  }
}

function overview(wallets: ChainOverview['wallets']): ChainOverview {
  return {
    cluster: 'devnet',
    program: { last_deploy_slot: null, upgrade_authority: null },
    accounts: [],
    wallets,
    faucet_sol_lamports: 1_000_000_000,
    faucet_token_base_units: 10_000_000,
    faucet_enabled: true,
    sponsor_settle_lamports: 3_944_360,
    read_at: 0
  }
}

describe('tokenUnits', () => {
  it('shows six-decimal base units without trailing zeros', () => {
    expect(tokenUnits(10_000_000)).toBe('10')
    expect(tokenUnits(1_234_500)).toBe('1.2345')
    expect(tokenUnits(0)).toBe('0')
  })
})

describe('rentSurplus', () => {
  it('is what sits above the rent-exempt minimum, never below zero', () => {
    expect(rentSurplus({ lamports: 30_300_000, rent_exempt_lamports: 890_880 })).toBe(29_409_120)
    expect(rentSurplus({ lamports: 0, rent_exempt_lamports: 890_880 })).toBe(0)
  })
})

describe('claimsLeft', () => {
  it('is bounded by the scarcest of SOL, USDC and EURC', () => {
    const faucet = account({
      lamports: 25_500_000_000,
      tokens: { usdc_base_units: 300_000_000, eurc_base_units: 75_000_000 }
    })
    expect(claimsLeft(overview([{ key: 'faucet', configured: true, account: faucet }]))).toBe(7)
  })

  it('treats a missing token account as none of that token', () => {
    const faucet = account({
      lamports: 25_000_000_000,
      tokens: { usdc_base_units: 300_000_000, eurc_base_units: null }
    })
    expect(claimsLeft(overview([{ key: 'faucet', configured: true, account: faucet }]))).toBe(0)
  })

  it('has no answer when the faucet key is not configured', () => {
    expect(claimsLeft(overview([{ key: 'faucet', configured: false, account: null }]))).toBeNull()
  })
})

describe('settlesLeft', () => {
  it('counts the settles the relayer can front', () => {
    const sponsor = account({ key: 'sponsor', lamports: 39_443_600 })
    expect(settlesLeft(overview([{ key: 'sponsor', configured: true, account: sponsor }]))).toBe(10)
    expect(settlesLeft(overview([]))).toBeNull()
  })
})
