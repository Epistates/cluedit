import { writable } from "svelte/store";

export interface StatusState {
  loading: boolean;
  message: string;
  progress: { current: number; total: number } | null;
  error: string | null;
}

export const statusStore = writable<StatusState>({
  loading: false,
  message: "",
  progress: null,
  error: null,
});

export function setLoading(message: string, progress?: { current: number; total: number }) {
  statusStore.set({
    loading: true,
    message,
    progress: progress || null,
    error: null,
  });
}

export function setError(error: string) {
  statusStore.set({
    loading: false,
    message: "",
    progress: null,
    error,
  });
}

export function setMessage(message: string) {
  statusStore.set({
    loading: false,
    message,
    progress: null,
    error: null,
  });
}

export function clearStatus() {
  statusStore.set({
    loading: false,
    message: "",
    progress: null,
    error: null,
  });
}
