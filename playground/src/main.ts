import { mount } from 'svelte';
import Playground from './components/Playground.svelte';

const app = mount(Playground, {
    target: document.getElementById('app')!,
    props: {
        config: {
            showToolbar: true,
            showOutput: true,
            height: '100%',
            enableShare: true,
            enableExamples: true,
            readUrlHash: true,
            fontSize: 14,
            layout: 'horizontal',
        }
    }
});

export default app;
