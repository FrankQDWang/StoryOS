export function requiredGlobalTeardown(operation: () => Promise<void>): () => Promise<void> {
  return async () => {
    try {
      await operation();
    } catch (error) {
      process.exitCode = 1;
      throw error;
    }
  };
}
