import { app } from 'electron';
import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { z } from 'zod';

const storedUserSchema = z.object({
  id: z.string().uuid(),
  email: z.string().email(),
  displayName: z.string().min(2).max(32),
  avatar: z.string().min(1).max(4),
  passwordHash: z.string().min(1),
  passwordSalt: z.string().min(1),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

const sessionSchema = z.object({
  token: z.string().min(1),
  userId: z.string().uuid(),
  createdAt: z.string().datetime(),
  expiresAt: z.string().datetime(),
});

const databaseSchema = z.object({
  users: z.array(storedUserSchema),
  session: sessionSchema.nullable(),
});

export type StoredUser = z.infer<typeof storedUserSchema>;
export type StoredSession = z.infer<typeof sessionSchema>;
type DatabaseShape = z.infer<typeof databaseSchema>;

function resolveDbPath(): string {
  const dir = path.join(app.getPath('userData'), 'data');
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
  return path.join(dir, 'auth.json');
}

function emptyDatabase(): DatabaseShape {
  return { users: [], session: null };
}

function readDatabase(): DatabaseShape {
  const filePath = resolveDbPath();
  if (!existsSync(filePath)) {
    return emptyDatabase();
  }

  try {
    const raw = JSON.parse(readFileSync(filePath, 'utf8')) as unknown;
    return databaseSchema.parse(raw);
  } catch {
    return emptyDatabase();
  }
}

function writeDatabase(database: DatabaseShape): void {
  const parsed = databaseSchema.parse(database);
  writeFileSync(resolveDbPath(), `${JSON.stringify(parsed, null, 2)}\n`, 'utf8');
}

export function listUsers(): StoredUser[] {
  return readDatabase().users;
}

export function findUserByEmail(email: string): StoredUser | null {
  const normalized = email.trim().toLowerCase();
  return listUsers().find((user) => user.email === normalized) ?? null;
}

export function findUserById(id: string): StoredUser | null {
  return listUsers().find((user) => user.id === id) ?? null;
}

export function createUser(input: {
  email: string;
  displayName: string;
  passwordHash: string;
  passwordSalt: string;
}): StoredUser {
  const database = readDatabase();
  const now = new Date().toISOString();
  const user: StoredUser = {
    id: randomUUID(),
    email: input.email.trim().toLowerCase(),
    displayName: input.displayName.trim(),
    avatar: input.displayName.trim().slice(0, 1).toUpperCase(),
    passwordHash: input.passwordHash,
    passwordSalt: input.passwordSalt,
    createdAt: now,
    updatedAt: now,
  };

  database.users.push(user);
  writeDatabase(database);
  return user;
}

export function updateUser(
  id: string,
  patch: Partial<Pick<StoredUser, 'displayName' | 'avatar' | 'updatedAt'>>,
): StoredUser | null {
  const database = readDatabase();
  const index = database.users.findIndex((user) => user.id === id);
  if (index < 0) {
    return null;
  }

  const current = database.users[index];
  const next: StoredUser = {
    ...current,
    ...patch,
    updatedAt: new Date().toISOString(),
  };
  database.users[index] = next;
  writeDatabase(database);
  return next;
}

export function getSession(): StoredSession | null {
  return readDatabase().session;
}

export function setSession(session: StoredSession | null): void {
  const database = readDatabase();
  database.session = session;
  writeDatabase(database);
}
