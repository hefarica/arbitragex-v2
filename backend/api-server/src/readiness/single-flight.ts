/** Coalesce overlapping reads only; never cache settled readiness/kill states.
 * Keys are dependency identities (e.g. a PG pool), not credentials or URLs.
 * Both success and failure remove the entry. A rejected shared operation never
 * creates an unhandled rejection in the cleanup path.
 */
export function createSingleFlight<K, V>() {
  const pending = new Map<K, Promise<V>>();
  return (key: K, read: () => Promise<V>): Promise<V> => {
    const current = pending.get(key);
    if (current) return current;
    const result = Promise.resolve().then(read);
    pending.set(key, result);
    const release = () => {
      if (pending.get(key) === result) pending.delete(key);
    };
    void result.then(release, release);
    return result;
  };
}
