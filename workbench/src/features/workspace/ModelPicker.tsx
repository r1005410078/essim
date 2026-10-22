import { useId, useState } from 'react';
import type { CollectionModel } from '../../shared/api/contracts';
/** 名称仅用于搜索与新建意图，绑定值仍是稳定 ID。 */
export function ModelPicker({ models, value, newName, onChange }: {
    models: CollectionModel[];
    value: string;
    newName: string;
    onChange: (id: string, name: string) => void;
}) {
    const [query, setQuery] = useState('');
    const [open, setOpen] = useState(false);
    const [active, setActive] = useState(0);
    const id = useId();
    const found = models.filter(m => m.name.toLowerCase().includes(query.toLowerCase()));
    const options = [...found.map(m => ({ id: m.id, name: m.name, label: `${m.name} · ${m.id}` })), ...(query.trim() ? [{ id: '__new__', name: query.trim(), label: `新建“${query.trim()}”并绑定` }] : [])];
    const choose = (i: number) => { const option = options[i]; if (!option)
        return; onChange(option.id, option.id === '__new__' ? option.name : ''); setOpen(false); setQuery(''); };
    const label = value === '__new__' ? `新建：${newName}` : models.find(m => m.id === value)?.name ?? '';
    return <div className="model-picker"><input aria-label="选择或新建采集模型" role="combobox" aria-expanded={open} aria-controls={id} aria-autocomplete="list" aria-activedescendant={open && options[active] ? `${id}-${active}` : undefined} value={open ? query : label} placeholder="搜索模型，或输入新模型名称" onFocus={() => { setOpen(true); setActive(0); }} onChange={e => { setQuery(e.target.value); setOpen(true); setActive(0); }} onKeyDown={e => { if (e.nativeEvent.isComposing)
        return; if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        setOpen(true);
        setActive(i => Math.max(0, Math.min(options.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1))));
    } if (e.key === 'Enter' && open) {
        e.preventDefault();
        choose(active);
    } if (e.key === 'Escape' && open) {
        e.preventDefault();
        e.stopPropagation();
        setOpen(false);
    } }} onBlur={() => setOpen(false)}/>{open && <div id={id} role="listbox" className="model-picker-options">{options.map((option, i) => <button key={option.id} id={`${id}-${i}`} type="button" role="option" aria-selected={i === active} onMouseDown={e => e.preventDefault()} onClick={() => choose(i)}>{option.label}</button>)}{!options.length && <span>输入名称可新建采集模型</span>}</div>}</div>;
}
