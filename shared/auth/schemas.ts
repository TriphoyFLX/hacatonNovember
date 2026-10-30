import { z } from 'zod';

export const registerSchema = z
  .object({
    displayName: z
      .string()
      .trim()
      .min(2, 'Имя слишком короткое')
      .max(32, 'Имя слишком длинное'),
    email: z.string().trim().email('Введите корректный email').max(120),
    password: z
      .string()
      .min(8, 'Пароль должен быть не короче 8 символов')
      .max(128, 'Пароль слишком длинный'),
    confirmPassword: z.string(),
  })
  .refine((value) => value.password === value.confirmPassword, {
    message: 'Пароли не совпадают',
    path: ['confirmPassword'],
  });

export const loginSchema = z.object({
  email: z.string().trim().email('Введите корректный email').max(120),
  password: z.string().min(1, 'Введите пароль').max(128),
});

export const updateProfileSchema = z.object({
  displayName: z
    .string()
    .trim()
    .min(2, 'Имя слишком короткое')
    .max(32, 'Имя слишком длинное'),
});

export type RegisterInput = z.infer<typeof registerSchema>;
export type LoginInput = z.infer<typeof loginSchema>;
export type UpdateProfileInput = z.infer<typeof updateProfileSchema>;

export interface PublicUser {
  id: string;
  email: string;
  displayName: string;
  avatar: string;
  createdAt: string;
  updatedAt: string;
}

export interface AuthSession {
  token: string;
  userId: string;
  createdAt: string;
  expiresAt: string;
}

export type AuthResult =
  | { ok: true; user: PublicUser; session: AuthSession }
  | { ok: false; error: string };

export type SessionResult =
  | { ok: true; user: PublicUser; session: AuthSession }
  | { ok: false; user: null; session: null };

export type ProfileResult =
  | { ok: true; user: PublicUser }
  | { ok: false; error: string };
