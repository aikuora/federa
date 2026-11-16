import { realpathSync } from 'node:fs';
import { homedir, userInfo } from 'node:os';

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g;

// Paths and names of the person become fixed placeholders. Any value under a key that names an
// account, an organization or an email, and the account's usage in `rate_limit_info`, become
// `redacted`. Ids inside the folders synced from the account, which name its organization, become
// zeros.
export function scrubber(workdir: string, config: string): (line: string) => string {
  const paths: [string, string][] = [
    [realpathSync(workdir), '/work'],
    [workdir, '/work'],
    [realpathSync(config), '/config'],
    [config, '/config'],
    [homedir(), '/home/person'],
  ];
  const name = userInfo().username;
  const email = /[\w.+-]+@[\w-]+(\.[\w-]+)+/g;
  const personal = /email|account|organization|org_?id|user_?id|rate_limit_info/i;
  const synced = /(plugins|skills)\/synced\/[^"]*/g;
  const zeros = '00000000-0000-0000-0000-000000000000';
  const scrubValue = (value: unknown, key = ''): unknown => {
    if (personal.test(key)) return 'redacted';
    if (typeof value === 'string') {
      let text = value;
      for (const [path, placeholder] of paths) text = text.replaceAll(path, placeholder);
      text = text.replace(synced, (path) => path.replace(UUID, zeros));
      return text.replaceAll(name, 'person').replace(email, 'person@example.com');
    }
    if (Array.isArray(value)) return value.map((item) => scrubValue(item, key));
    if (value !== null && typeof value === 'object') {
      return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, scrubValue(v, k)]));
    }
    return value;
  };
  return (line) => JSON.stringify(scrubValue(JSON.parse(line)));
}
