<script lang="ts">
    import { Navbar, NavBrand, NavLi, NavUl } from 'flowbite-svelte';
</script>

<Navbar class="bg-primary-100 dark:bg-primary-700 py-0" fluid={false}>
    <NavBrand>
        <span class="self-center whitespace-nowrap text-xl font-semibold dark:text-white">Open Photo AI</span>
    </NavBrand>

    <NavUl ulClass="py-2">
        <NavLi href="/">Home</NavLi>
        <NavLi href="/about">About</NavLi>
        <NavLi href="/docs/components/navbar">Navbar</NavLi>
        <NavLi href="/pricing">Pricing</NavLi>
        <NavLi href="/contact">Contact</NavLi>
    </NavUl>
</Navbar>
