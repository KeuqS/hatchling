import type { AppContext } from '../../app/context.js';
import { icon } from '../../components/icons.js';
import { menu } from '../../components/ui.js';
export function avatarMenu(ctx: AppContext): HTMLElement {
    const control = menu('Edit agent profile', [
        { label: 'Change avatar', icon: 'people', action: () => ctx.prefillComposer('Change my avatar to ') },
        { label: 'Edit name', icon: 'edit', action: () => ctx.prefillComposer('Change my name to ') },
    ]);
    control.classList.add('avatar-edit-menu');
    const trigger = control.querySelector('summary');
    trigger?.replaceChildren(icon('edit', 16));
    trigger?.classList.add('avatar-edit');
    trigger?.removeAttribute('title');
    trigger?.removeAttribute('data-tooltip');
    return control;
}
