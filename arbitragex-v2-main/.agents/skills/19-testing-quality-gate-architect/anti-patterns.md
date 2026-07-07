# Antipatrones Prohibidos

## Antipatrón 1: Testing Implementation Details
```tsx
// 🔴 PROHIBIDO
test('Component state updates', () => {
  const wrapper = shallow(<MyComponent />);
  wrapper.instance().setState({ clicked: true }); // Mal. Nunca interactuar directamente con el estado interno de React.
  expect(wrapper.state('clicked')).toBe(true);
});
```
