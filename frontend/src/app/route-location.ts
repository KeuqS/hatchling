/** URL history is an enhancement, not a prerequisite for local navigation.
 * Script-enabled attachment/iframe previews may have an opaque origin and deny
 * pushState/replaceState. The controller still owns and renders the selected view.
 * Do not escape the frame, change its parent URL, or relax its sandbox to fix this.
 */
export function writeRouteHash(hash: string, replace = false, target: Pick<Window, 'history' | 'location'> = window): boolean {
    if (!hash.startsWith('#'))
        return false;
    try {
        if (target.location.hash !== hash)
            target.history[replace ? 'replaceState' : 'pushState'](null, '', hash);
        return true;
    }
    catch {
        // The in-memory route remains usable; this host cannot provide URL history.
        return false;
    }
}
