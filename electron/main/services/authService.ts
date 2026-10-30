import { randomBytes, scryptSync, timingSafeEqual } from 'node:crypto';
import {
  loginSchema,
  registerSchema,
  updateProfileSchema,
  type AuthResult,
  type LoginInput,
  type ProfileResult,
  type PublicUser,
  type RegisterInput,
  type SessionResult,
  type UpdateProfileInput,
} from '../../../shared/auth/schemas';
import {
  createUser,
  findUserByEmail,
  findUserById,
  getSession,
  setSession,
  updateUser,
  type StoredUser,
} from '../db/userStore';

const SESSION_TTL_MS = 1000 * 60 * 60 * 24 * 30;

function toPublicUser(user: StoredUser): PublicUser {
  return {
    id: user.id,
    email: user.email,
    displayName: user.displayName,
    avatar: user.avatar,
    createdAt: user.createdAt,
    updatedAt: user.updatedAt,
  };
}

function hashPassword(password: string, salt: string): string {
  return scryptSync(password, salt, 64).toString('hex');
}

function verifyPassword(password: string, salt: string, expectedHash: string): boolean {
  const actual = Buffer.from(hashPassword(password, salt), 'hex');
  const expected = Buffer.from(expectedHash, 'hex');
  if (actual.length !== expected.length) {
    return false;
  }
  return timingSafeEqual(actual, expected);
}

function createSession(userId: string) {
  const now = Date.now();
  return {
    token: randomBytes(32).toString('hex'),
    userId,
    createdAt: new Date(now).toISOString(),
    expiresAt: new Date(now + SESSION_TTL_MS).toISOString(),
  };
}

function readValidSessionUser(): { user: PublicUser; session: NonNullable<ReturnType<typeof getSession>> } | null {
  const session = getSession();
  if (!session) {
    return null;
  }

  if (Date.parse(session.expiresAt) <= Date.now()) {
    setSession(null);
    return null;
  }

  const user = findUserById(session.userId);
  if (!user) {
    setSession(null);
    return null;
  }

  return { user: toPublicUser(user), session };
}

export function register(raw: RegisterInput): AuthResult {
  const parsed = registerSchema.safeParse(raw);
  if (!parsed.success) {
    return { ok: false, error: parsed.error.issues[0]?.message ?? 'Некорректные данные' };
  }

  const { email, password, displayName } = parsed.data;
  if (findUserByEmail(email)) {
    return { ok: false, error: 'Пользователь с таким email уже существует' };
  }

  const salt = randomBytes(16).toString('hex');
  const passwordHash = hashPassword(password, salt);
  const user = createUser({
    email,
    displayName,
    passwordHash,
    passwordSalt: salt,
  });
  const session = createSession(user.id);
  setSession(session);

  return { ok: true, user: toPublicUser(user), session };
}

export function login(raw: LoginInput): AuthResult {
  const parsed = loginSchema.safeParse(raw);
  if (!parsed.success) {
    return { ok: false, error: parsed.error.issues[0]?.message ?? 'Некорректные данные' };
  }

  const user = findUserByEmail(parsed.data.email);
  if (!user || !verifyPassword(parsed.data.password, user.passwordSalt, user.passwordHash)) {
    return { ok: false, error: 'Неверный email или пароль' };
  }

  const session = createSession(user.id);
  setSession(session);
  return { ok: true, user: toPublicUser(user), session };
}

export function logout(): { ok: true } {
  setSession(null);
  return { ok: true };
}

export function getCurrentSession(): SessionResult {
  const active = readValidSessionUser();
  if (!active) {
    return { ok: false, user: null, session: null };
  }
  return { ok: true, user: active.user, session: active.session };
}

export function updateProfile(raw: UpdateProfileInput): ProfileResult {
  const parsed = updateProfileSchema.safeParse(raw);
  if (!parsed.success) {
    return { ok: false, error: parsed.error.issues[0]?.message ?? 'Некорректные данные' };
  }

  const active = readValidSessionUser();
  if (!active) {
    return { ok: false, error: 'Сессия не найдена. Войдите снова.' };
  }

  const updated = updateUser(active.user.id, {
    displayName: parsed.data.displayName,
    avatar: parsed.data.displayName.slice(0, 1).toUpperCase(),
  });

  if (!updated) {
    return { ok: false, error: 'Не удалось обновить профиль' };
  }

  return { ok: true, user: toPublicUser(updated) };
}
