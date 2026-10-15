import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { creditsApi } from "@/lib/api/credits";

export const creditsQueryKey = ["credits", "dashboard"] as const;

export function useCreditsDashboard() {
  return useQuery({
    queryKey: creditsQueryKey,
    queryFn: creditsApi.dashboard,
    staleTime: 10_000,
    refetchInterval: 30_000,
    refetchOnWindowFocus: true,
  });
}

export function useCreditsAction<TInput, TOutput>(action: (input: TInput) => Promise<TOutput>) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: action,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: creditsQueryKey }),
    onError: (error) => toast.error(error instanceof Error ? error.message : String(error)),
  });
}
