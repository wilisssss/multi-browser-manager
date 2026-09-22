import type { ComponentType } from "react";
import { SiBitcoin, SiDiscord, SiEthereum, SiFacebook, SiGmail, SiGoogle, SiInstagram, SiReddit, SiTelegram, SiTiktok, SiX } from "@icons-pack/react-simple-icons";
import { KeyRound } from "lucide-react";

/**
 * Platform templates for the per-profile credential manager. Each template
 * defines which fields the form shows; `usernameLabel` is the first identity
 * field. Secrets (password / seed phrase) are stored in the OS keychain.
 */
export interface PlatformTemplate {
  id: string;
  name: string;
  icon: ComponentType<{ className?: string }>;
  usernameLabel: string;
  usernamePlaceholder: string;
  hasPassword: boolean;
  hasSeedPhrase: boolean;
  hasEvmAddress: boolean;
}

export const PLATFORMS: PlatformTemplate[] = [
  {
    id: "x",
    name: "X (Twitter)",
    icon: SiX,
    usernameLabel: "Username",
    usernamePlaceholder: "@handle",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
  {
    id: "facebook",
    name: "Facebook",
    icon: SiFacebook,
    usernameLabel: "Email",
    usernamePlaceholder: "name@mail.com",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
  {
    id: "discord",
    name: "Discord",
    icon: SiDiscord,
    usernameLabel: "Email",
    usernamePlaceholder: "name@mail.com",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
  {
    id: "google",
    name: "Google",
    icon: SiGoogle,
    usernameLabel: "Email",
    usernamePlaceholder: "name@gmail.com",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
  {
    id: "gmail",
    name: "Gmail (recovery)",
    icon: SiGmail,
    usernameLabel: "Email",
    usernamePlaceholder: "name@gmail.com",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
  {
    id: "instagram",
    name: "Instagram",
    icon: SiInstagram,
    usernameLabel: "Username",
    usernamePlaceholder: "@handle",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
  {
    id: "tiktok",
    name: "TikTok",
    icon: SiTiktok,
    usernameLabel: "Email / Username",
    usernamePlaceholder: "email or @handle",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
  {
    id: "reddit",
    name: "Reddit",
    icon: SiReddit,
    usernameLabel: "Username",
    usernamePlaceholder: "u/username",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
  {
    id: "telegram",
    name: "Telegram",
    icon: SiTelegram,
    usernameLabel: "Phone / Username",
    usernamePlaceholder: "+62... or @handle",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
  {
    id: "wallet",
    name: "EVM Wallet",
    icon: SiEthereum,
    usernameLabel: "Account name",
    usernamePlaceholder: "Main wallet",
    hasPassword: false,
    hasSeedPhrase: true,
    hasEvmAddress: true,
  },
  {
    id: "bitcoin",
    name: "Bitcoin Wallet",
    icon: SiBitcoin,
    usernameLabel: "Account name",
    usernamePlaceholder: "Main wallet",
    hasPassword: false,
    hasSeedPhrase: true,
    hasEvmAddress: false,
  },
  {
    id: "custom",
    name: "Custom",
    icon: KeyRound,
    usernameLabel: "Username / Email",
    usernamePlaceholder: "username or email",
    hasPassword: true,
    hasSeedPhrase: false,
    hasEvmAddress: false,
  },
];

export function findPlatform(id: string): PlatformTemplate {
  return PLATFORMS.find((p) => p.id === id) ?? PLATFORMS[PLATFORMS.length - 1];
}
