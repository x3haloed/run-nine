"""Mouse request states; elapsed time and immediate post-read VBL timing are raw only."""
import copy
import itertools

PHASES = ('baseline', 'immediate', 'settled', 'after_system_task', 'second_uncoupled', 'recoupled')
CONTEXT = ('hide', 'origin', 'mask', 'which')
GRID = list(itertools.product(range(2), range(2), range(18), range(4)))
POINTS = ('mtemp', 'raw_mouse', 'mouse', 'get_mouse')
SCALARS = ('flags_before_get', 'flags_after_get', 'prefix', 'suffix')
TARGETS = [[40, 60], [200, 210], [1, 638], [478, 1]]


def rows_valid(rows, raw=False, native=False):
    if not isinstance(rows, list) or len(rows) != len(GRID):
        raise ValueError('Incomplete mouse request matrix')
    for row, context in zip(rows, GRID):
        fields = set(CONTEXT) | {'requested', *PHASES}
        if raw and native:
            fields.add('elapsed_ticks')
        if not isinstance(row, dict) or set(row) != fields:
            raise ValueError('Missing mouse request observation')
        if any(type(row[k]) is not int for k in CONTEXT) or tuple(row[k] for k in CONTEXT) != context:
            raise ValueError('Changed mouse request inputs')
        if row['requested'] != TARGETS[context[-1]]:
            raise ValueError('Changed target point')
        if raw and native and (type(row['elapsed_ticks']) is not int or row['elapsed_ticks'] < 2):
            raise ValueError('Incomplete native delay')
        for phase in PHASES:
            state = row[phase]
            fields = set(POINTS + SCALARS)
            if not raw and phase == 'immediate':
                fields.remove('flags_after_get')
            if not isinstance(state, dict) or set(state) != fields:
                raise ValueError('Missing mouse phase observation')
            for k in POINTS:
                point = state[k]
                if not isinstance(point, list) or len(point) != 2 or any(type(x) is not int or not -32768 <= x <= 32767 for x in point):
                    raise ValueError('Invalid mouse point')
            for k in set(SCALARS) & fields:
                limit = 65535 if k.startswith('flags') else 0xffffffff
                if type(state[k]) is not int or not 0 <= state[k] <= limit:
                    raise ValueError('Invalid mouse scalar')
            if raw and state['flags_after_get'] not in (state['flags_before_get'], state['flags_before_get'] & 255):
                raise ValueError('Unexpected cursor flag transition during observation')


def validate(value):
    if not isinstance(value, dict) or set(value) != {'schema', 'probe', 'mouse'} or value['schema'] != 1 or value['probe'] != 'mouse-coupling-v1':
        raise ValueError('Wrong mouse coupling profile')
    if not isinstance(value['mouse'], dict) or set(value['mouse']) != {'records'}:
        raise ValueError('Missing mouse records')
    rows_valid(value['mouse']['records'])


def project(raw):
    if not isinstance(raw, dict) or set(raw) != {'schema', 'probe', 'records'} or raw['schema'] != 1 or raw['probe'] not in ('mouse-coupling-raw-v1', 'mouse-coupling-runtime-raw-v1'):
        raise ValueError('Wrong raw mouse capture')
    native = raw['probe'] == 'mouse-coupling-raw-v1'
    rows_valid(raw['records'], raw=True, native=native)
    rows = copy.deepcopy(raw['records'])
    for row in rows:
        row.pop('elapsed_ticks', None)
        # A native VBL may clear CrsrNew between the two flag reads around
        # GetMouse. Keep both readings raw; compare the subsequent settled state.
        del row['immediate']['flags_after_get']
    result = {'schema': 1, 'probe': 'mouse-coupling-v1', 'mouse': {'records': rows}}
    validate(result)
    return result
