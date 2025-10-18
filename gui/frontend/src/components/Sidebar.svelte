<script lang="ts">
    import { Sidebar, SidebarGroup, SidebarItem } from 'flowbite-svelte';
</script>

<Sidebar backdrop={false} isOpen={true} class="fixed left-auto right-0 w-64">
    <SidebarGroup>
        <SidebarItem label="Dashboard" href="/" />
        <SidebarItem label="Settings" href="/settings" />
    </SidebarGroup>
</Sidebar>
